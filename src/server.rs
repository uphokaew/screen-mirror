use super::config::Config;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn};

pub struct ServerManager;

impl ServerManager {
    pub fn new() -> Result<Self> {
        // Verify ADB is accessible
        let status = Command::new("adb")
            .arg("start-server")
            .status()
            .context("Failed to run 'adb'. Is it in your PATH?")?;

        if !status.success() {
            anyhow::bail!("adb start-server failed with exit code: {}", status);
        }
        Ok(Self)
    }

    pub fn start_server(&mut self, config: &Config, serial: Option<&str>) -> Result<()> {
        let serial = serial.map(|s| s.to_string());

        // 1. Check devices
        let output = Command::new("adb")
            .args(["devices"])
            .output()
            .context("Failed to list devices")?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        if !output_str.contains("\tdevice") {
            anyhow::bail!(
                "No ADB devices found. Connect your phone via USB and enable USB Debugging."
            );
        }

        // 2. Determine Target Serial and ensure connection
        let mut target_serial = serial.clone();

        if let Some(s) = &serial
            && !output_str.contains(s)
        {
            info!("Device {} not found in ADB. Attempting to connect...", s);
            // Try connect if IP
            if s.contains('.') {
                let _ = Command::new("adb").args(["connect", s]).status();
                // Re-check
                let check_output = Command::new("adb").arg("devices").output()?;
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

        // 3. Push scrcpy-server.jar
        let jar_name = "scrcpy-server";
        let jar_paths = [jar_name.to_string(), "scrcpy-server".to_string()];

        let local_jar = jar_paths
            .iter()
            .find(|p| Path::new(p).exists())
            .context("scrcpy-server not found! Please place it in the application folder.")?;

        info!("Pushing {} to device...", local_jar);

        let mut push_cmd = Command::new("adb");
        if let Some(s) = &target_serial {
            push_cmd.args(["-s", s]);
        }

        let status = push_cmd
            .args(["push", local_jar, "/data/local/tmp/scrcpy-server"])
            .status()
            .context("Failed to push server jar")?;

        if !status.success() {
            anyhow::bail!("Failed to push scrcpy-server.jar to device.");
        }

        // 4. Setup port forwarding (Forward PC port 5555 to Device socket)
        info!("Setting up port forwarding...");
        let mut forward_cmd = Command::new("adb");
        if let Some(s) = &target_serial {
            forward_cmd.args(["-s", s]);
        }
        // adb forward tcp:5555 localabstract:scrcpy
        let status = forward_cmd
            .args(["forward", "tcp:5555", "localabstract:scrcpy"])
            .status()
            .context("Failed to run adb forward")?;

        if !status.success() {
            warn!("adb forward failed.");
        }

        // 5. Start server
        info!("Starting server...");
        let bitrate_arg = format!("video_bit_rate={}", config.video.bitrate * 1000000);
        let tunnel_forward = "tunnel_forward=true";
        let control = "control=false"; // FORCED: Output only
        let audio = "audio=false"; // FORCED: Disable audio for now (requires 2nd socket)
        let video = "video=true";
        let max_size = format!("max_size={}", config.video.max_size);
        let cleanup = "cleanup=true"; // Clean up on exit

        let cmd_string = format!(
            "CLASSPATH=/data/local/tmp/scrcpy-server app_process / com.genymobile.scrcpy.Server 3.3.3 {} {} {} {} {} {} {}",
            tunnel_forward, bitrate_arg, control, audio, video, max_size, cleanup
        );

        let serial_clone = target_serial.clone();
        let cmd_string_clone = cmd_string.clone();

        thread::spawn(move || {
            let mut server_cmd = Command::new("adb");
            if let Some(s) = &serial_clone {
                server_cmd.args(["-s", s]);
            }

            info!("Executing server command on device...");
            let status = server_cmd.args(["shell", &cmd_string_clone]).status();

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
        thread::sleep(Duration::from_millis(2000));

        Ok(())
    }
}
