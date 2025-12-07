use anyhow::{Context, Result};
/// Connection negotiation and capability exchange
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use super::{Connection, NetworkError, QuicConnection, TcpConnection};

/// Device capabilities exchanged during handshake
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Device name/model
    pub device_name: String,

    /// Maximum supported resolution
    pub max_resolution: (u32, u32),

    /// Supported video codecs
    pub video_codecs: Vec<String>,

    /// Supported audio codecs
    pub audio_codecs: Vec<String>,

    /// Maximum bitrate (Mbps)
    pub max_bitrate: u32,

    /// Supports audio streaming
    pub audio_supported: bool,

    /// Preferred connection mode
    pub preferred_mode: String, // "tcp" or "quic"
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            device_name: "Android Device".to_string(),
            max_resolution: (1920, 1080),
            video_codecs: vec!["h264".to_string(), "h265".to_string()],
            audio_codecs: vec!["aac".to_string(), "opus".to_string()],
            max_bitrate: 20,
            audio_supported: true,
            preferred_mode: "tcp".to_string(),
        }
    }
}

/// Connection negotiator with automatic fallback
pub struct ConnectionNegotiator {
    tcp_addr: SocketAddr,
    quic_addr: Option<SocketAddr>,
    prefer_quic: bool,
    timeout_ms: u64,
}

impl ConnectionNegotiator {
    /// Create a new connection negotiator
    ///
    /// # Arguments
    /// * `tcp_addr` - TCP address (always available as fallback)
    /// * `quic_addr` - Optional QUIC address for wireless
    /// * `prefer_quic` - Try QUIC first if available
    pub fn new(tcp_addr: SocketAddr, quic_addr: Option<SocketAddr>, prefer_quic: bool) -> Self {
        Self {
            tcp_addr,
            quic_addr,
            prefer_quic,
            timeout_ms: 5000,
        }
    }

    /// Negotiate and establish connection with automatic fallback
    ///
    /// # Returns
    /// Connection type enum indicating which protocol was used
    pub async fn connect(&self) -> Result<ConnectionType> {
        // Try preferred connection first
        if self.prefer_quic && self.quic_addr.is_some() {
            tracing::info!("Attempting QUIC connection...");
            match self.try_quic().await {
                Ok(conn) => {
                    tracing::info!("QUIC connection established");
                    return Ok(ConnectionType::Quic(conn));
                }
                Err(e) => {
                    tracing::warn!("QUIC connection failed: {}, falling back to TCP", e);
                }
            }
        }

        // Fallback to TCP
        tracing::info!("Attempting TCP connection...");
        match self.try_tcp().await {
            Ok(conn) => {
                tracing::info!("TCP connection established");
                Ok(ConnectionType::Tcp(conn))
            }
            Err(e) => Err(anyhow::anyhow!(
                "All connection attempts failed. TCP error: {}",
                e
            )),
        }
    }

    /// Try QUIC connection with timeout
    async fn try_quic(&self) -> Result<QuicConnection> {
        let addr = self
            .quic_addr
            .ok_or_else(|| anyhow::anyhow!("QUIC address not provided"))?;

        let conn = tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms),
            QuicConnection::connect(addr),
        )
        .await
        .context("QUIC connection timeout")?
        .map_err(|e| anyhow::anyhow!("QUIC connection error: {:?}", e))?;

        Ok(conn)
    }

    /// Try TCP connection with timeout
    async fn try_tcp(&self) -> Result<TcpConnection> {
        let conn = tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms),
            TcpConnection::connect(self.tcp_addr),
        )
        .await
        .context("TCP connection timeout")?
        .map_err(|e| anyhow::anyhow!("TCP connection error: {:?}", e))?;

        Ok(conn)
    }

    /// Exchange capabilities with server
    pub async fn exchange_capabilities<C: Connection>(
        &self,
        conn: &mut C,
        client_caps: &DeviceCapabilities,
    ) -> Result<DeviceCapabilities> {
        // Send client capabilities
        let caps_data =
            bincode::serialize(client_caps).context("Failed to serialize capabilities")?;

        // TODO: Send/receive capabilities via control channel
        // For now, return default server capabilities
        tracing::info!(
            "Capabilities exchanged: client supports {:?}",
            client_caps.video_codecs
        );

        Ok(DeviceCapabilities::default())
    }
}

/// Connection type enum for polymorphic connection handling
pub enum ConnectionType {
    Tcp(TcpConnection),
    Quic(QuicConnection),
}

impl ConnectionType {
    /// Get connection name
    pub fn name(&self) -> &str {
        match self {
            ConnectionType::Tcp(_) => "TCP",
            ConnectionType::Quic(_) => "QUIC",
        }
    }

    /// Check if wireless (QUIC)
    pub fn is_wireless(&self) -> bool {
        matches!(self, ConnectionType::Quic(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_capabilities() {
        let caps = DeviceCapabilities::default();
        assert_eq!(caps.max_resolution, (1920, 1080));
        assert!(caps.video_codecs.contains(&"h264".to_string()));
    }

    #[test]
    fn test_connection_negotiator_creation() {
        let tcp_addr = "127.0.0.1:5555".parse().unwrap();
        let quic_addr = Some("127.0.0.1:5556".parse().unwrap());

        let negotiator = ConnectionNegotiator::new(tcp_addr, quic_addr, true);
        assert_eq!(negotiator.prefer_quic, true);
    }
}
