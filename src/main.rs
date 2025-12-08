#![allow(deprecated)] // Suppress winit 0.30 deprecation warnings until full refactor
use anyhow::Result;
use clap::Parser;
use scrcpy_custom::{
    audio::{decoder::HardwareAudioDecoder, player::AudioPlayer},
    config::{Config, ConnectionMode},
    network::*,
    video::{
        decoder::{DecodedFrame, HardwareVideoDecoder, PixelFormat},
        renderer::VideoRenderer,
    },
};
use std::net::{IpAddr, SocketAddr};
use std::sync::mpsc;
use std::thread;
// use std::time::Duration;
use tracing::{error, info, warn};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

/// Ultra-low latency screen mirroring application
#[derive(Parser, Debug, Clone)]
#[command(name = "scrcpy-custom")]
#[command(about = "High-performance screen mirroring from Android to PC", long_about = None)]
struct Args {
    /// Connection mode: tcp or quic
    #[arg(short, long, value_enum, default_value = "tcp")]
    mode: ConnectionModeArg,

    /// Server IP address
    #[arg(long, default_value = "127.0.0.1")]
    host: IpAddr,

    /// Server port
    #[arg(short, long, default_value_t = 5555)]
    port: u16,

    /// Video bitrate in Mbps
    #[arg(short, long, default_value_t = 8)]
    bitrate: u32,

    /// Enable hardware acceleration
    #[arg(long, default_value_t = true)]
    hw_accel: bool,

    /// Hardware decoder (auto, nvdec, qsv, vaapi)
    #[arg(long, default_value = "auto")]
    hw_decoder: String,

    /// Disable audio
    #[arg(long, default_value_t = false)]
    no_audio: bool,

    /// Max video size (0 = native)
    #[arg(long, default_value_t = 0)]
    max_size: u16,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ConnectionModeArg {
    Tcp,
    Quic,
}

impl From<ConnectionModeArg> for ConnectionMode {
    fn from(mode: ConnectionModeArg) -> Self {
        match mode {
            ConnectionModeArg::Tcp => ConnectionMode::Tcp,
            ConnectionModeArg::Quic => ConnectionMode::Quic,
        }
    }
}

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let args_clone = args.clone();

    info!("Starting scrcpy-custom");
    info!(
        "Mode: {:?}, Host: {}, Port: {}",
        args.mode, args.host, args.port
    );

    // Setup Winit Event Loop
    let event_loop = EventLoop::new().unwrap();

    // Create window using winit 0.30 API
    let window_attributes = Window::default_attributes()
        .with_title("scrcpy-custom")
        .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 576.0));

    let window = event_loop.create_window(window_attributes).unwrap();

    // Initialize Video Renderer
    let mut renderer = VideoRenderer::new(&window)?;

    // Channel to send decoded frames from network thread to UI thread
    let (frame_tx, frame_rx) = mpsc::channel::<DecodedFrame>();

    // Spawn Network/Decoding Thread
    thread::spawn(move || {
        // Create a new Tokio runtime for async network operations
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            // Build configuration
            let mut config = Config::default();
            config.connection.mode = args_clone.mode.into();
            config.connection.host = args_clone.host;
            config.connection.port = args_clone.port;
            config.video.bitrate = args_clone.bitrate;
            config.video.hw_accel = args_clone.hw_accel;
            config.video.hw_decoder = args_clone.hw_decoder.clone();
            config.video.max_size = args_clone.max_size;
            config.performance.adaptive_bitrate = false; // Forced false as no control socket

            config.audio.enabled = !args_clone.no_audio;

            if let Err(e) = run_app(config, frame_tx).await {
                error!("Application error: {}", e);
            }
        });
    });

    // Run Event Loop
    let _ = event_loop.run(move |event, target| {
        target.set_control_flow(ControlFlow::Poll); // Check for events continuously

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                target.exit();
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                let _ = renderer.resize(size.width, size.height);
            }
            Event::AboutToWait => {
                // Check for new frames
                // Process all available frames, render the last one (skip dropping frames for now to keep sync simple)
                // Or better: render every frame we get?
                // For now, let's just drain the channel and render the newest one to minimize latency
                let mut last_frame = None;
                while let Ok(frame) = frame_rx.try_recv() {
                    last_frame = Some(frame);
                }

                if let Some(frame) = last_frame
                    && let Err(e) = renderer.render(&frame)
                {
                    error!("Render error: {}", e);
                }
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // Normally we'd render here, but we render immediately on AboutToWait for lowest latency
            }
            _ => {}
        }
    });

    Ok(())
}

// Network logic moved here
async fn run_app(mut config: Config, frame_tx: mpsc::Sender<DecodedFrame>) -> Result<()> {
    // Attempt to auto-start server via ADB
    info!("Checking matching scrcpy-server via ADB...");
    let mut adb_success = false;

    match scrcpy_custom::server::ServerManager::new() {
        Ok(mut manager) => {
            let serial = if !config.connection.host.is_loopback() {
                Some(config.connection.host.to_string())
            } else {
                None
            };

            if let Err(e) = manager.start_server(&config, serial.as_deref()) {
                warn!("ADB Server setup failed: {}.", e);
            } else {
                info!("Server setup successful via ADB!");
                adb_success = true;
            }
        }
        Err(e) => {
            warn!("Could not connect to ADB: {}. Proceeding without ADB.", e);
        }
    }

    // If ADB setup was successful, we MUST connect to localhost because we used 'adb forward'
    if adb_success {
        info!("Redirecting connection to localhost:5555 (tunnel via ADB)");
        config.connection.host = "127.0.0.1".parse().unwrap();
        config.connection.port = 5555;
    }

    let addr = SocketAddr::new(config.connection.host, config.connection.port);
    info!("Connecting to {}...", addr);

    let mode = config.connection.mode;
    match mode {
        ConnectionMode::Tcp => {
            info!("Using TCP connection");
            run_with_connection::<TcpConnection>(addr, config, frame_tx).await
        }
        ConnectionMode::Quic => {
            info!("Using QUIC connection");
            run_with_connection::<QuicConnection>(addr, config, frame_tx).await
        }
    }
}

fn handle_connection_error(e: &anyhow::Error) {
    let error_msg = e.to_string();
    if error_msg.contains("10061") || error_msg.contains("Connection refused") {
        error!("--------------------------------------------------");
        error!("CONNECTION REFUSED");
        error!("1. Ensure 'adb' is in your PATH.");
        error!("2. Ensure 'scrcpy-server' is in the same folder.");
        error!("3. Check if 'adb devices' lists your device.");
        error!("--------------------------------------------------");
    }
}

async fn run_with_connection<C: Connection>(
    addr: SocketAddr,
    config: Config,
    frame_tx: mpsc::Sender<DecodedFrame>,
) -> Result<()> {
    // Connect to server
    let mut connection = C::connect(addr).await.map_err(|e| {
        handle_connection_error(&anyhow::anyhow!(e.to_string()));
        anyhow::anyhow!("Failed to connect: {}", e)
    })?;

    info!("Connected successfully!");

    // Initialize Decoders
    let output_format = PixelFormat::RGBA; // WGPU prefers RGBA usually
    let mut video_decoder = HardwareVideoDecoder::new(&config.video.hw_decoder, output_format)?;
    info!("Initialized Video Decoder: {}", video_decoder.info());

    // Initialize Audio (Opus/AAC default usually Opus for scrcpy audio)
    // Note: Scrcpy server usually sends Opus for audio enabled.
    // We'll initialize lazily or default to Opus 48kHz stereo
    let mut audio_decoder = HardwareAudioDecoder::new("opus", 48000, 2).or_else(|_| {
        warn!("Opus decoder not found, trying AAC");
        HardwareAudioDecoder::new("aac", 48000, 2)
    });

    let mut audio_player = if audio_decoder.is_ok() {
        match AudioPlayer::new(48000, 2, 50) {
            // 50ms jitter buffer
            Ok(player) => Some(player),
            Err(e) => {
                warn!("Failed to initialize audio player: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Main receive loop
    info!("Starting receive loop...");
    loop {
        let packet = match connection.recv().await {
            Ok(p) => p,
            Err(e) => {
                error!("Receive error: {}", e);
                break;
            }
        };

        match packet.packet_type {
            PacketType::Video => {
                match video_decoder.decode(&packet.data, packet.pts) {
                    Ok(Some(frame)) => {
                        // Send frame to UI thread
                        if let Err(e) = frame_tx.send(frame) {
                            error!("Failed to send frame to UI: {}", e);
                            break; // UI thread likely dead
                        }
                    }
                    Ok(None) => {} // Need more data
                    Err(e) => error!("Video decoding error: {}", e),
                }
            }
            PacketType::Audio => {
                if let (Ok(decoder), Some(player)) = (&mut audio_decoder, &mut audio_player) {
                    match decoder.decode(&packet.data, packet.pts) {
                        Ok(Some(audio_frame)) => {
                            if let Err(e) = player.play(audio_frame) {
                                error!("Audio playback error: {}", e);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => error!("Audio decoding error: {}", e),
                    }
                }
            }
            PacketType::Control => {
                // Ignore control messages
            }
            PacketType::Handshake => {
                info!("Received handshake packet");
                // In a full impl, we'd parse device name/size here
            }
            PacketType::Fec => {}
        }
    }
    info!("Connection closed");
    Ok(())
}
