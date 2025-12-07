use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::net::SocketAddr;
use thiserror::Error;

pub mod fec;
pub mod negotiation;
pub mod protocol;
pub mod quic;
pub mod tcp;

pub use fec::{FecDecoder, FecEncoder};
pub use negotiation::{ConnectionNegotiator, ConnectionType, DeviceCapabilities};
pub use protocol::{ControlMessage, Packet, PacketType};
pub use quic::QuicConnection;
pub use tcp::TcpConnection;

/// Network errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Timeout")]
    Timeout,

    #[error("QUIC error: {0}")]
    Quic(String),
}

pub type Result<T> = std::result::Result<T, NetworkError>;

/// Connection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    Tcp,
    Quic,
}

/// Abstract connection trait for both TCP and QUIC
#[async_trait]
pub trait Connection: Send + Sync {
    /// Connect to the server
    async fn connect(addr: SocketAddr) -> Result<Self>
    where
        Self: Sized;

    /// Receive a packet from the connection
    async fn recv(&mut self) -> Result<Packet>;

    /// Send a control message to the server
    async fn send_control(&mut self, msg: ControlMessage) -> Result<()>;

    /// Get network statistics
    fn stats(&self) -> NetworkStats;

    /// Close the connection
    async fn close(&mut self) -> Result<()>;
}

/// Network statistics for monitoring and adaptive bitrate
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkStats {
    /// Round-trip time in milliseconds
    pub rtt_ms: f64,

    /// Packet loss percentage (0.0 - 100.0)
    pub packet_loss: f64,

    /// Current bandwidth estimate (Mbps)
    pub bandwidth_mbps: f64,

    /// Bytes received
    pub bytes_received: u64,

    /// Packets received
    pub packets_received: u64,

    /// Packets lost
    pub packets_lost: u64,
}

impl NetworkStats {
    /// Calculate network quality score (0.0 = poor, 1.0 = excellent)
    pub fn quality_score(&self) -> f64 {
        let rtt_score = (1.0 - (self.rtt_ms / 500.0).min(1.0)).max(0.0);
        let loss_score = (1.0 - (self.packet_loss / 5.0).min(1.0)).max(0.0);
        (rtt_score * 0.6 + loss_score * 0.4)
    }
}
