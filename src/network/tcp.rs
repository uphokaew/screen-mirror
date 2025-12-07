use super::{Connection, ControlMessage, NetworkError, NetworkStats, Packet, PacketType, Result};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// TCP connection for wired (USB/ADB) connectivity
pub struct TcpConnection {
    stream: TcpStream,
    stats: NetworkStats,
    last_rtt_check: Instant,
    recv_buffer: BytesMut,
}

impl TcpConnection {
    /// Buffer size for receiving data (1MB for low latency)
    const RECV_BUFFER_SIZE: usize = 1024 * 1024;

    /// Timeout for connection attempts
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

    /// Timeout for read operations
    const READ_TIMEOUT: Duration = Duration::from_secs(3);

    /// Create a new TCP connection
    pub async fn new(addr: SocketAddr) -> Result<Self> {
        let stream = timeout(Self::CONNECT_TIMEOUT, TcpStream::connect(addr))
            .await
            .map_err(|_| NetworkError::Timeout)?
            .map_err(|e| NetworkError::ConnectionFailed(e.to_string()))?;

        // Configure for low latency
        stream.set_nodelay(true)?;

        Ok(Self {
            stream,
            stats: NetworkStats::default(),
            last_rtt_check: Instant::now(),
            recv_buffer: BytesMut::with_capacity(Self::RECV_BUFFER_SIZE),
        })
    }

    /// Read exact number of bytes from stream with timeout
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        timeout(Self::READ_TIMEOUT, self.stream.read_exact(buf))
            .await
            .map_err(|_| NetworkError::Timeout)?
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    NetworkError::ConnectionClosed
                } else {
                    NetworkError::Io(e)
                }
            })
            .map(|_| ())
    }

    /// Measure RTT by sending a ping control message
    async fn measure_rtt(&mut self) -> Result<()> {
        // Only measure every second to avoid overhead
        if self.last_rtt_check.elapsed() < Duration::from_secs(1) {
            return Ok(());
        }

        let start = Instant::now();

        // Send a control message (in real implementation, we'd wait for ACK)
        let msg = ControlMessage::Ack { seq: 0 };
        let data = msg
            .to_bytes()
            .map_err(|e| NetworkError::Protocol(e.to_string()))?;

        let packet = Packet::new(PacketType::Control, 0, 0, data);

        self.stream.write_all(&packet.to_bytes()).await?;

        // Estimate RTT (simplified - real implementation would wait for server ACK)
        self.stats.rtt_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.last_rtt_check = Instant::now();

        Ok(())
    }
}

#[async_trait]
impl Connection for TcpConnection {
    async fn connect(addr: SocketAddr) -> Result<Self> {
        Self::new(addr).await
    }

    async fn recv(&mut self) -> Result<Packet> {
        // Read packet header (17 bytes)
        let mut header = [0u8; Packet::HEADER_SIZE];
        self.read_exact(&mut header).await?;

        // Parse length from header (last 4 bytes)
        let len = u32::from_le_bytes([header[13], header[14], header[15], header[16]]) as usize;

        // Validate packet size (max 10MB for safety)
        if len > 10 * 1024 * 1024 {
            return Err(NetworkError::Protocol("Packet too large".to_string()));
        }

        // Read payload
        let mut payload = vec![0u8; len];
        self.read_exact(&mut payload).await?;

        // Combine header and payload
        let mut full_packet = BytesMut::with_capacity(Packet::HEADER_SIZE + len);
        full_packet.extend_from_slice(&header);
        full_packet.extend_from_slice(&payload);

        // Parse packet
        let packet = Packet::from_bytes(full_packet.freeze())
            .map_err(|e| NetworkError::Protocol(e.to_string()))?;

        // Update stats
        self.stats.bytes_received += (Packet::HEADER_SIZE + len) as u64;
        self.stats.packets_received += 1;

        // Periodically measure RTT
        let _ = self.measure_rtt().await;

        Ok(packet)
    }

    async fn send_control(&mut self, msg: ControlMessage) -> Result<()> {
        let data = msg
            .to_bytes()
            .map_err(|e| NetworkError::Protocol(e.to_string()))?;

        let packet = Packet::new(PacketType::Control, 0, 0, data);

        self.stream.write_all(&packet.to_bytes()).await?;
        self.stream.flush().await?;

        Ok(())
    }

    fn stats(&self) -> NetworkStats {
        self.stats
    }

    async fn close(&mut self) -> Result<()> {
        self.stream.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_serialization() {
        let data = Bytes::from_static(b"test data");
        let packet = Packet::new(PacketType::Video, 12345, 1, data.clone());

        let serialized = packet.to_bytes();
        let deserialized = Packet::from_bytes(serialized.freeze()).unwrap();

        assert_eq!(deserialized.packet_type, PacketType::Video);
        assert_eq!(deserialized.pts, 12345);
        assert_eq!(deserialized.seq, 1);
        assert_eq!(deserialized.data, data);
    }
}
