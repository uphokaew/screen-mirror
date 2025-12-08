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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
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

    // Interactive Mode Selection if no arguments provided
    // This allows the user to choose between Wired (USB) and Wireless without typing commands
    let mut args = Args::parse();

    if std::env::args().len() <= 1 {
        println!("========================================");
        println!("      Scrcpy-Custom Mode Selection      ");
        println!("========================================");
        println!("1. Wired Connection (USB) [Default]");
        println!("2. Wireless Connection (TCP/WiFi)");
        println!("========================================");
        println!("Enter choice (1/2): ");

        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            match input.trim() {
                "2" => {
                    args.mode = ConnectionModeArg::Tcp; // Currently both use TCP, but this might imply IP input later
                    // Ideally for wireless we might want to ask for IP
                    println!("Enter Device IP (e.g. 192.168.1.100): ");
                    let mut ip_input = String::new();
                    if std::io::stdin().read_line(&mut ip_input).is_ok() {
                        if let Ok(ip) = ip_input.trim().parse::<IpAddr>() {
                            args.host = ip;
                        } else {
                            println!("Invalid IP. Using default.");
                        }
                    }
                }
                _ => {
                    println!("Selected: Wired Connection (USB)");
                    // Default args.mode is already TCP (USB tunnel)
                }
            }
        }
    }

    info!("Starting scrcpy-custom");
    info!(
        "Mode: {:?}, Host: {}, Port: {}",
        args.mode, args.host, args.port
    );

    let args_clone = args.clone();

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

    // Shutdown signal
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

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

            if args_clone.no_audio {
                config.audio.enabled = false;
            } else {
                config.audio.enabled = true;
                // Smart Codec Negotiation
                // Try to initialize Opus decoder. If it fails, fallback to AAC.
                // We do this check BEFORE connecting/starting server so we can tell the server what to send.
                if HardwareAudioDecoder::new("opus", 48000, 2).is_ok() {
                    info!("Client supports Opus audio. Requesting Opus from server.");
                    config.audio.codec = scrcpy_custom::config::AudioCodec::Opus;
                } else if HardwareAudioDecoder::new("aac", 48000, 2).is_ok() {
                    warn!("Client does not support Opus. Requesting AAC from server.");
                    config.audio.codec = scrcpy_custom::config::AudioCodec::Aac;
                } else {
                    warn!("No supported audio decoder found (Opus/AAC). Disabling audio.");
                    config.audio.enabled = false;
                }
            }

            if let Err(e) = run_app(config, frame_tx, running_clone).await {
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
                running.store(false, Ordering::SeqCst);
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
                let mut last_frame = None;
                while let Ok(frame) = frame_rx.try_recv() {
                    last_frame = Some(frame);
                }

                if let Some(frame) = last_frame {
                    // Auto-resize window if video size changes (orientation change or first frame)
                    // We use the renderer's current tracking to detect change
                    let current_video_size = renderer.current_video_size();
                    if current_video_size != Some((frame.width, frame.height)) {
                        let inner_size = renderer.window().inner_size();
                        if inner_size.width > 0 && inner_size.height > 0 {
                            let w = frame.width as f64;
                            let h = frame.height as f64;
                            let aspect = w / h;

                            // Simple heuristic:
                            // 1. If rotation (Portrait <-> Landscape), flip window dimensions
                            // 2. Otherwise, adjust width to match new aspect ratio, keeping height
                            let new_size = if let Some((old_w, old_h)) = current_video_size {
                                let old_aspect = old_w as f64 / old_h as f64;
                                let is_landscape = aspect > 1.0;
                                let was_landscape = old_aspect > 1.0;

                                if is_landscape != was_landscape {
                                    // Rotation: Flip window
                                    winit::dpi::PhysicalSize::new(
                                        inner_size.height,
                                        inner_size.width,
                                    )
                                } else {
                                    // Resolution change: Adjust width to match aspect
                                    let new_width = (inner_size.height as f64 * aspect) as u32;
                                    winit::dpi::PhysicalSize::new(new_width, inner_size.height)
                                }
                            } else {
                                // First frame: Set reasonable default height (e.g. 800 or current) and adjust width
                                // But don't make it larger than screen.
                                // Let's simplify: Scale to e.g. 1/3 of video if it's huge, or just match current height
                                let target_height = if inner_size.height < 100 {
                                    800.0
                                } else {
                                    inner_size.height as f64
                                };
                                let new_width = (target_height * aspect) as u32;
                                winit::dpi::PhysicalSize::new(new_width, target_height as u32)
                            };

                            let _ = renderer.window().request_inner_size(new_size);
                        }
                    }

                    if let Err(e) = renderer.render(&frame) {
                        error!("Render error: {}", e);
                    }
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
async fn run_app(
    mut config: Config,
    frame_tx: mpsc::Sender<DecodedFrame>,
    running: Arc<AtomicBool>,
) -> Result<()> {
    // Attempt to auto-start server via ADB
    info!("Checking matching scrcpy-server via ADB...");
    let mut adb_success = false;

    match scrcpy_custom::server::ServerManager::new().await {
        Ok(mut manager) => {
            let serial = if !config.connection.host.is_loopback() {
                Some(config.connection.host.to_string())
            } else {
                None
            };

            if let Err(e) = manager.start_server(&config, serial.as_deref()).await {
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
            run_with_connection::<TcpConnection>(addr, config, frame_tx, running).await
        }
        ConnectionMode::Quic => {
            info!("Using QUIC connection");
            run_with_connection::<QuicConnection>(addr, config, frame_tx, running).await
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
    running: Arc<AtomicBool>,
) -> Result<()> {
    // Connect to server
    let mut connection = C::connect(addr, config.audio.enabled).await.map_err(|e| {
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
        match AudioPlayer::new(48000, 2, config.performance.jitter_buffer_ms) {
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
        if !running.load(Ordering::Relaxed) {
            info!("Shutdown signal received");
            break;
        }

        // Use tokio timeout for select! -like behavior with cancellation
        // But since we removed read timeout, we might block forever stuck in recv().
        // We need a way to check 'running' while waiting.
        // Option 1: Timeout short loop? No, excessive.
        // Option 2: tokio::select! with a cancellation token.
        // For now, simplicity: Check before recv. If recv blocks forever, forced process exit kills it anyway.
        // But to be "Check running flag" compliant, we should ideally use select.
        // Let's rely on process exit for hard kill, but check flag for cooperative exit (e.g. if we add UI stop button)

        // Actually, for "safest possible", we want to ensure we don't crash on exit.

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
