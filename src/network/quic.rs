use super::protocol::FecPacket;
use super::{Connection, ControlMessage, NetworkError, NetworkStats, Packet, PacketType, Result};
use async_trait::async_trait;
use bytes::Bytes;
use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, VarInt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// QUIC connection for wireless (WiFi) connectivity
pub struct QuicConnection {
    connection: quinn::Connection,
    recv_stream: Arc<Mutex<Option<RecvStream>>>,
    send_stream: Arc<Mutex<Option<SendStream>>>,
    stats: NetworkStats,
    fec_decoder: FecDecoder,
    last_seq: u32,
}

impl QuicConnection {
    /// Create a new QUIC connection
    pub async fn new(addr: SocketAddr) -> Result<Self> {
        // Configure QUIC client
        let mut client_config = ClientConfig::with_platform_verifier();

        // Configure transport for low latency
        let mut transport_config = quinn::TransportConfig::default();

        // Reduce initial RTT estimate for faster connection
        transport_config.initial_rtt(Duration::from_millis(100));

        // Enable keep-alive to detect connection loss quickly
        transport_config.keep_alive_interval(Some(Duration::from_secs(1)));

        // Set max idle timeout
        transport_config.max_idle_timeout(Some(VarInt::from_u32(30_000).into()));

        // Configure congestion control for low latency (similar to BBR)
        // quinn uses Cubic by default, which is good for throughput

        // Configure receive window for high throughput
        transport_config.receive_window(VarInt::from_u32(8 * 1024 * 1024));
        transport_config.send_window(8 * 1024 * 1024);

        // Set stream receive window
        transport_config.stream_receive_window(VarInt::from_u32(2 * 1024 * 1024));

        client_config.transport_config(Arc::new(transport_config));

        // Create endpoint
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| NetworkError::Quic(e.to_string()))?;

        endpoint.set_default_client_config(client_config);

        // Connect to server
        let connection = endpoint
            .connect(addr, "localhost")
            .map_err(|e| NetworkError::Quic(e.to_string()))?
            .await
            .map_err(|e| NetworkError::Quic(e.to_string()))?;

        Ok(Self {
            connection,
            recv_stream: Arc::new(Mutex::new(None)),
            send_stream: Arc::new(Mutex::new(None)),
            stats: NetworkStats::default(),
            fec_decoder: FecDecoder::new(10), // 10% redundancy
            last_seq: 0,
        })
    }

    /// Receive data via unreliable datagram (lowest latency for video)
    async fn recv_datagram(&self) -> Result<Bytes> {
        self.connection
            .read_datagram()
            .await
            .map_err(|e| NetworkError::Quic(e.to_string()))
    }

    /// Receive data via reliable stream (for control messages)
    async fn recv_stream_data(&self, buf: &mut [u8]) -> Result<usize> {
        let mut stream_lock = self.recv_stream.lock().await;

        if stream_lock.is_none() {
            // Accept a new stream
            let stream = self
                .connection
                .accept_uni()
                .await
                .map_err(|e| NetworkError::Quic(e.to_string()))?;
            *stream_lock = Some(stream);
        }

        let stream = stream_lock.as_mut().unwrap();

        stream
            .read(buf)
            .await
            .map_err(|e| NetworkError::Quic(e.to_string()))?
            .ok_or(NetworkError::ConnectionClosed)
    }

    /// Send control message via reliable stream
    async fn send_stream_data(&self, data: &[u8]) -> Result<()> {
        let mut stream_lock = self.send_stream.lock().await;

        if stream_lock.is_none() {
            // Open a new stream
            let stream = self
                .connection
                .open_uni()
                .await
                .map_err(|e| NetworkError::Quic(e.to_string()))?;
            *stream_lock = Some(stream);
        }

        let stream = stream_lock.as_mut().unwrap();

        stream
            .write_all(data)
            .await
            .map_err(|e| NetworkError::Quic(e.to_string()))?;

        Ok(())
    }

    /// Update network statistics from QUIC connection
    fn update_stats(&mut self) {
        let stats = self.connection.stats();

        // Get RTT from QUIC path stats
        self.stats.rtt_ms = stats.path.rtt.as_millis() as f64;

        // Calculate packet loss
        let total_packets = self.stats.packets_received + self.stats.packets_lost;
        if total_packets > 0 {
            self.stats.packet_loss =
                (self.stats.packets_lost as f64 / total_packets as f64) * 100.0;
        }

        // Estimate bandwidth (simplified)
        // In a real implementation, we'd track bytes over time
        self.stats.bandwidth_mbps = (stats.path.cwnd as f64 * 8.0) / (self.stats.rtt_ms * 125.0);
    }
}

#[async_trait]
impl Connection for QuicConnection {
    async fn connect(addr: SocketAddr) -> Result<Self> {
        Self::new(addr).await
    }

    async fn recv(&mut self) -> Result<Packet> {
        // Receive datagram (used for video/audio - low latency, loss-tolerant)
        let data = self.recv_datagram().await?;

        // Try to parse as packet
        let packet =
            Packet::from_bytes(data.clone()).map_err(|e| NetworkError::Protocol(e.to_string()))?;

        // Check for packet loss
        if packet.seq > self.last_seq + 1 {
            let lost = packet.seq - self.last_seq - 1;
            self.stats.packets_lost += lost as u64;
        }
        self.last_seq = packet.seq;

        // Update stats
        self.stats.bytes_received += data.len() as u64;
        self.stats.packets_received += 1;
        self.update_stats();

        // Handle FEC if this is a FEC packet
        if packet.packet_type == PacketType::Fec {
            // Decode FEC packet and try to recover lost packets
            if let Some(recovered) = self.fec_decoder.process_fec(&packet.data) {
                // Return recovered packet
                return Ok(recovered);
            } else {
                // FEC packet processed but no recovery yet, wait for next packet
                return self.recv().await;
            }
        }

        // Add to FEC decoder for potential recovery
        self.fec_decoder.add_packet(packet.seq, packet.data.clone());

        Ok(packet)
    }

    async fn send_control(&mut self, msg: ControlMessage) -> Result<()> {
        let data = msg
            .to_bytes()
            .map_err(|e| NetworkError::Protocol(e.to_string()))?;

        // Send control messages via reliable stream
        self.send_stream_data(&data).await
    }

    fn stats(&self) -> NetworkStats {
        self.stats
    }

    async fn close(&mut self) -> Result<()> {
        self.connection
            .close(VarInt::from_u32(0), b"client shutdown");
        Ok(())
    }
}

/// FEC (Forward Error Correction) decoder using Reed-Solomon
/// Allows recovery of lost packets without retransmission
struct FecDecoder {
    redundancy_percent: u8,
    blocks: HashMap<u32, FecBlock>,
    last_cleanup: Instant,
}

struct FecBlock {
    block_id: u32,
    data_packets: HashMap<u8, Bytes>,
    parity_packets: HashMap<u8, Bytes>,
    data_count: u8,
    parity_count: u8,
    created_at: Instant,
}

impl FecDecoder {
    fn new(redundancy_percent: u8) -> Self {
        Self {
            redundancy_percent,
            blocks: HashMap::new(),
            last_cleanup: Instant::now(),
        }
    }

    /// Add a data packet to the FEC decoder
    fn add_packet(&mut self, seq: u32, data: Bytes) {
        // In a real implementation, we'd group packets into blocks
        // and store them for FEC recovery

        // Cleanup old blocks every 5 seconds
        if self.last_cleanup.elapsed() > Duration::from_secs(5) {
            self.cleanup_old_blocks();
        }
    }

    /// Process a FEC packet and attempt to recover lost packets
    fn process_fec(&mut self, fec_data: &Bytes) -> Option<Packet> {
        // Parse FEC packet
        let fec_packet = FecPacket::from_bytes(fec_data.clone()).ok()?;

        // Get or create FEC block
        let block = self
            .blocks
            .entry(fec_packet.block_id)
            .or_insert_with(|| FecBlock {
                block_id: fec_packet.block_id,
                data_packets: HashMap::new(),
                parity_packets: HashMap::new(),
                data_count: fec_packet.data_count,
                parity_count: fec_packet.parity_count,
                created_at: Instant::now(),
            });

        // Store parity packet
        block
            .parity_packets
            .insert(fec_packet.index, fec_packet.data);

        // Try to recover using Reed-Solomon
        // This is a simplified placeholder - real implementation would use
        // reed-solomon-erasure crate to recover lost packets

        // For now, return None (no recovery)
        None
    }

    /// Remove blocks older than 10 seconds
    fn cleanup_old_blocks(&mut self) {
        self.blocks
            .retain(|_, block| block.created_at.elapsed() < Duration::from_secs(10));
        self.last_cleanup = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fec_decoder() {
        let mut decoder = FecDecoder::new(10);

        let data = Bytes::from_static(b"test");
        decoder.add_packet(1, data.clone());
        decoder.add_packet(2, data.clone());

        // Cleanup should not remove recent blocks
        decoder.cleanup_old_blocks();
        assert_eq!(decoder.blocks.len(), 0); // No blocks created without FEC packets
    }
}
