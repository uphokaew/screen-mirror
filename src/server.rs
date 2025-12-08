use super::config::Config;
use crate::assets::Assets;
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{error, info, warn};

pub struct ServerManager;

impl ServerManager {
    pub async fn new() -> Result<Self> {
        // Verify ADB is accessible
        let adb_path = Assets::get_adb_path()?;
        let status = Command::new(&adb_path)
            .arg("start-server")
            .status()
            .await
            .context("Failed to run 'adb'. Is it in your PATH?")?;

        if !status.success() {
            anyhow::bail!("adb start-server failed with exit code: {}", status);
        }
        Ok(Self)
    }

    pub async fn start_server(&mut self, config: &Config, serial: Option<&str>) -> Result<()> {
        let serial = serial.map(|s| s.to_string());

        // 1. Check devices
        let adb_path = Assets::get_adb_path()?;
        let output = Command::new(&adb_path)
            .args(["devices"])
            .output()
            .await
            .context("Failed to list devices")?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        if !output_str.contains("\tdevice") {
            anyhow::bail!(
                "No ADB devices found. Connect your phone via USB and enable USB Debugging."
            );
        }

        // 2. Determine Target Serial and ensure connection
        let mut target_serial = serial.clone();

        if let Some(s) = &serial {
            if !output_str.contains(s) {
                info!("Device {} not found in ADB. Attempting to connect...", s);
                // Try connect if IP
                if s.contains('.') {
                    let _ = Command::new(&adb_path).args(["connect", s]).status().await;
                    // Re-check
                    let check_output = Command::new(&adb_path).arg("devices").output().await?;
                    let check_str = String::from_utf8_lossy(&check_output.stdout);
                    if !check_str.contains(s) {
                        // Fallback check: If exactly one device exists (e.g. USB), use it
                        let lines: Vec<&str> = check_str
                            .lines()
                            .filter(|l| l.contains("\tdevice"))
                            .collect();
                        if lines.len() == 1 {
                            let fallback = lines[0].split('\t').next().unwrap_or("").to_string();
                            if !fallback.is_empty() {
                                warn!(
                                    "Target {} not reachable. Falling back to connected device: {}",
                                    s, fallback
                                );
                                target_serial = Some(fallback);
                            }
                        } else {
                            warn!(
                                "Target {} not found and multiple/no other devices available.",
                                s
                            );
                        }
                    }
                }
            }
        }

        // 3. Push scrcpy-server.jar
        let local_jar = Assets::get_server_path()?;

        info!("Pushing {:?} to device...", local_jar);

        let mut push_cmd = Command::new(&adb_path);
        if let Some(s) = &target_serial {
            push_cmd.args(["-s", s]);
        }

        let status = push_cmd
            .arg("push")
            .arg(local_jar)
            .arg("/data/local/tmp/scrcpy-server")
            .status()
            .await
            .context("Failed to push server jar")?;

        if !status.success() {
            anyhow::bail!("Failed to push scrcpy-server.jar to device.");
        }

        // 4. Setup port forwarding (Forward PC port 5555 to Device socket)
        info!("Setting up port forwarding...");
        let mut forward_cmd = Command::new(&adb_path);
        if let Some(s) = &target_serial {
            forward_cmd.args(["-s", s]);
        }
        // adb forward tcp:5555 localabstract:scrcpy
        let status = forward_cmd
            .args(["forward", "tcp:5555", "localabstract:scrcpy"])
            .status()
            .await
            .context("Failed to run adb forward")?;

        if !status.success() {
            warn!("adb forward failed.");
        }

        // 5. Start server
        info!("Starting server...");
        let bitrate_arg = format!("video_bit_rate={}", config.video.bitrate * 1000000);
        let tunnel_forward = "tunnel_forward=true";
        let control = "control=false"; // FORCED: Output only
        let audio = format!("audio={}", config.audio.enabled);
        let audio_codec = format!("audio_codec={}", config.audio.codec.to_server_arg());
        let audio_dup = "audio_dup=false"; // output sound to computer only
        let video = "video=true";
        let max_size = format!("max_size={}", config.video.max_size);
        let cleanup = "cleanup=true"; // Clean up on exit

        let cmd_string = format!(
            "CLASSPATH=/data/local/tmp/scrcpy-server app_process / com.genymobile.scrcpy.Server 3.3.3 {} {} {} {} {} {} {} {} {}",
            tunnel_forward,
            bitrate_arg,
            control,
            audio,
            audio_codec,
            audio_dup,
            video,
            max_size,
            cleanup
        );

        let serial_clone = target_serial.clone();

        tokio::spawn(async move {
            let mut server_cmd = match Assets::get_adb_path() {
                Ok(p) => Command::new(p),
                Err(_) => Command::new("adb"), // Fallback unlikely to work if get_adb_path failed before
            };
            if let Some(s) = &serial_clone {
                server_cmd.args(["-s", s]);
            }

            use std::process::Stdio;

            info!("Executing server command on device...");
            let mut child = server_cmd
                .args(["shell", &cmd_string])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true) // Ensure process is killed when the task/handle drops
                .spawn()
                .expect("Failed to spawn server command");

            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            // Spawn log readers
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    info!("[SERVER] {}", line);
                }
            });

            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    warn!("[SERVER ERR] {}", line);
                }
            });

            let status = child.wait().await;

            match status {
                Ok(s) => {
                    if !s.success() {
                        error!("Server process exited with error code: {}", s);
                    } else {
                        info!("Server process exited normally.");
                    }
                }
                Err(e) => error!("Failed to run server command: {}", e),
            }
        });

        // Give it a moment to initialize
        tokio::time::sleep(Duration::from_millis(2000)).await;

        Ok(())
    }
}
