use anyhow::Result;
use clap::Parser;
use scrcpy_custom::{
    config::{Config, ConnectionMode},
    control::BitrateController,
    network::*,
};
use std::net::{IpAddr, SocketAddr};
use tracing::{error, info};
use tracing_subscriber;

/// Ultra-low latency screen mirroring application
#[derive(Parser, Debug)]
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

    /// Enable adaptive bitrate
    #[arg(long, default_value_t = true)]
    adaptive_bitrate: bool,
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

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    info!("Starting scrcpy-custom");
    info!(
        "Mode: {:?}, Host: {}, Port: {}",
        args.mode, args.host, args.port
    );

    // Build configuration
    let mut config = Config::default();
    config.connection.mode = args.mode.into();
    config.connection.host = args.host;
    config.connection.port = args.port;
    config.video.bitrate = args.bitrate;
    config.video.hw_accel = args.hw_accel;
    config.video.hw_decoder = args.hw_decoder;
    config.performance.adaptive_bitrate = args.adaptive_bitrate;

    // Run application
    if let Err(e) = run_app(config).await {
        error!("Application error: {}", e);
        return Err(e);
    }

    Ok(())
}

async fn run_app(config: Config) -> Result<()> {
    let addr = SocketAddr::new(config.connection.host, config.connection.port);

    info!("Connecting to {}...", addr);

    // Create connection based on mode
    let mode = config.connection.mode;

    match mode {
        ConnectionMode::Tcp => {
            info!("Using TCP connection (wired/USB)");
            run_with_connection::<TcpConnection>(addr, config).await
        }
        ConnectionMode::Quic => {
            info!("Using QUIC connection (wireless/WiFi)");
            run_with_connection::<QuicConnection>(addr, config).await
        }
    }
}

async fn run_with_connection<C: Connection>(addr: SocketAddr, config: Config) -> Result<()> {
    // Connect to server
    let mut connection = C::connect(addr)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;

    info!("Connected successfully!");

    // Initialize adaptive bitrate controller
    let mut bitrate_controller = if config.performance.adaptive_bitrate {
        Some(BitrateController::new(
            config.video.bitrate,
            2,  // min 2 Mbps
            20, // max 20 Mbps
        ))
    } else {
        None
    };

    // Main receive loop
    info!("Starting receive loop...");
    loop {
        // Receive packet
        let packet = match connection.recv().await {
            Ok(p) => p,
            Err(e) => {
                error!("Receive error: {}", e);
                break;
            }
        };

        // Process packet based on type
        match packet.packet_type {
            PacketType::Video => {
                info!(
                    "Received video packet: seq={}, pts={}, size={} bytes, keyframe={}",
                    packet.seq,
                    packet.pts,
                    packet.data.len(),
                    packet.is_keyframe()
                );

                // TODO: Decode and render video frame
            }
            PacketType::Audio => {
                info!(
                    "Received audio packet: seq={}, pts={}, size={} bytes",
                    packet.seq,
                    packet.pts,
                    packet.data.len()
                );

                // TODO: Decode and play audio
            }
            PacketType::Control => {
                info!("Received control packet");

                // TODO: Handle control messages
            }
            PacketType::Handshake => {
                info!("Received handshake packet");

                // TODO: Parse server capabilities
            }
            PacketType::Fec => {
                // FEC packets are handled internally by connection
            }
        }

        // Update adaptive bitrate
        if let Some(ref mut controller) = bitrate_controller {
            let stats = connection.stats();
            if let Some(control_msg) = controller.update(&stats) {
                info!(
                    "Adjusting bitrate to {} Mbps (RTT: {:.1}ms, Loss: {:.2}%)",
                    controller.current_bitrate(),
                    stats.rtt_ms,
                    stats.packet_loss
                );

                if let Err(e) = connection.send_control(control_msg).await {
                    error!("Failed to send control message: {}", e);
                }
            }
        }

        // For demo purposes, exit after 100 packets
        if packet.seq > 100 {
            info!("Demo completed, exiting...");
            break;
        }
    }

    // Close connection
    connection
        .close()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to close connection: {}", e))?;

    info!("Connection closed");
    Ok(())
}
