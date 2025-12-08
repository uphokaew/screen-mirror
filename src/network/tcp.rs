use super::{Connection, ControlMessage, NetworkError, NetworkStats, Packet, PacketType, Result};
use async_trait::async_trait;
// use bytes::BytesMut;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// TCP connection for wired (USB/ADB) connectivity
pub struct TcpConnection {
    stream: TcpStream,
    stats: NetworkStats,
}

impl TcpConnection {
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
        })
    }

    /// Read exact number of bytes from stream (blocking indefinitely)
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.stream
            .read_exact(buf)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    NetworkError::ConnectionClosed
                } else {
                    NetworkError::Io(e)
                }
            })
            .map(|_| ())
    }

    /// Read exact number of bytes with timeout (for handshake)
    async fn read_exact_timeout(&mut self, buf: &mut [u8]) -> Result<()> {
        timeout(Self::READ_TIMEOUT, self.read_exact(buf))
            .await
            .map_err(|_| NetworkError::Timeout)?
    }
}

#[async_trait]
impl Connection for TcpConnection {
    async fn connect(addr: SocketAddr) -> Result<Self> {
        let mut connection = Self::new(addr).await?;

        // Handshake: Read device name (64 bytes)
        let mut device_name = [0u8; 64];
        connection.read_exact_timeout(&mut device_name).await?;
        let name = String::from_utf8_lossy(&device_name);
        tracing::info!("Connected to device: {}", name.trim_matches(char::from(0)));

        // Consume 1 dummy byte? (Observed 0x00 before CodecID)
        let mut dummy = [0u8; 1];
        connection.read_exact_timeout(&mut dummy).await?;
        tracing::info!("Consuming dummy byte: 0x{:02X}", dummy[0]);

        // Scrcpy Video Stream Header: CodecID (4) + Width (4) + Height (4) = 12 bytes
        let mut meta = [0u8; 12];
        connection.read_exact_timeout(&mut meta).await?;
        let codec_id = u32::from_be_bytes(meta[0..4].try_into().unwrap());
        let width = u32::from_be_bytes(meta[4..8].try_into().unwrap());
        let height = u32::from_be_bytes(meta[8..12].try_into().unwrap());
        tracing::info!(
            "Video Stream Metadata: CodecID=0x{:08X}, Width={}, Height={}",
            codec_id,
            width,
            height
        );

        Ok(connection)
    }

    async fn recv(&mut self) -> Result<Packet> {
        // Scrcpy Protocol:
        // [8 bytes PTS] [4 bytes LEN] [LEN bytes DATA]
        // All big-endian

        let mut header = [0u8; 12];
        self.read_exact(&mut header).await?;

        let pts = u64::from_be_bytes(header[0..8].try_into().unwrap()) as i64;
        let len = u32::from_be_bytes(header[8..12].try_into().unwrap()) as usize;

        // Validate packet size (max 20MB for safety - keyframes can be large)
        if len > 20 * 1024 * 1024 {
            return Err(NetworkError::Protocol(format!(
                "Packet too large: {} bytes",
                len
            )));
        }

        let mut payload = vec![0u8; len];
        self.read_exact(&mut payload).await?;

        // Update stats
        self.stats.bytes_received += (12 + len) as u64;
        self.stats.packets_received += 1;

        // Construct Video Packet
        Ok(Packet::new(
            PacketType::Video,
            pts,
            0,
            bytes::Bytes::from(payload),
        ))
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
    use bytes::Bytes;

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
