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
    // We only need write access to video stream for control messages
    control_writer: tokio::net::tcp::OwnedWriteHalf,
    // Receiver for multiplexed packets (Video + Audio)
    packet_rx: tokio::sync::mpsc::Receiver<Result<Packet>>,
    stats: NetworkStats,
}

impl TcpConnection {
    /// Timeout for connection attempts
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

    /// Timeout for read operations (Handshake only)
    const READ_TIMEOUT: Duration = Duration::from_secs(10);

    /// Helper to read a packet from a stream
    async fn read_packet(
        reader: &mut tokio::net::tcp::OwnedReadHalf,
        packet_type: PacketType,
    ) -> Result<Packet> {
        // [PTS 8][LEN 4][DATA LEN]
        let mut header = [0u8; 12];
        reader.read_exact(&mut header).await?;

        let pts = u64::from_be_bytes(header[0..8].try_into().unwrap()) as i64;
        let len = u32::from_be_bytes(header[8..12].try_into().unwrap()) as usize;

        if len > 20 * 1024 * 1024 {
            return Err(NetworkError::Protocol(format!("Packet too large: {} bytes", len)).into());
        }

        let mut payload = vec![0u8; len];
        reader.read_exact(&mut payload).await?;

        Ok(Packet::new(
            packet_type,
            pts,
            0,
            bytes::Bytes::from(payload),
        ))
    }
}

#[async_trait]
impl Connection for TcpConnection {
    async fn connect(addr: SocketAddr, enable_audio: bool) -> Result<Self> {
        // 1. Connect Video Socket
        let video_stream = timeout(Self::CONNECT_TIMEOUT, TcpStream::connect(addr))
            .await
            .map_err(|_| NetworkError::Timeout)?
            .map_err(|e| NetworkError::ConnectionFailed(e.to_string()))?;
        video_stream.set_nodelay(true)?;

        // 2 & 3. Concurrent Initialization: Handshake (Video) and Connect (Audio)
        let (mut video_reader, control_writer) = video_stream.into_split();
        // We do this concurrently to avoid Deadlocks (Server waiting for Audio vs Client waiting for Name)
        // and Race Conditions (Server sending Name immediately).

        let video_reader_ref = &mut video_reader; // Borrow for async block

        let handshake_future = async {
            tracing::info!("Waiting for device name (Video Socket)...");
            let mut device_name = [0u8; 64];
            match timeout(
                Self::READ_TIMEOUT,
                video_reader_ref.read_exact(&mut device_name),
            )
            .await
            {
                Ok(Ok(_)) => {
                    let name = String::from_utf8_lossy(&device_name);
                    tracing::info!("Connected to device: {}", name.trim_matches(char::from(0)));
                    Ok(())
                }
                Ok(Err(e)) => {
                    tracing::error!("Failed to read device name: {}", e);
                    Err(NetworkError::ConnectionFailed(format!(
                        "Video Handshake Error: {}",
                        e
                    )))
                }
                Err(_) => {
                    tracing::error!("Timeout waiting for device name! Is the server running?");
                    Err(NetworkError::Timeout)
                }
            }
        };

        let audio_connect_future = async {
            if enable_audio {
                tracing::info!("Audio enabled. Connecting to audio socket...");
                match timeout(Self::CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
                    Ok(Ok(stream)) => {
                        if let Err(e) = stream.set_nodelay(true) {
                            tracing::warn!("Failed to set nodelay on audio socket: {}", e);
                        }
                        tracing::info!("Audio socket connected!");
                        let (reader, _) = stream.into_split();
                        Some(reader)
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(
                            "Failed to connect to audio socket: {}. Continuing without audio.",
                            e
                        );
                        None
                    }
                    Err(_) => {
                        tracing::warn!(
                            "Timeout connecting to audio socket. Continuing without audio."
                        );
                        None
                    }
                }
            } else {
                tracing::info!("Audio disabled. Skipping audio socket.");
                None
            }
        };

        // Run both concurrently
        let (handshake_res, audio_reader_res) =
            tokio::join!(handshake_future, audio_connect_future);

        // Check handshake result
        handshake_res?;
        let audio_reader = audio_reader_res;

        // 4, 5, 6. Concurrent Metadata Read
        // We read video metadata and audio metadata concurrently to prevent ordering issues
        let video_metadata_future = async {
            // Read Dummy Byte (Video)
            tracing::info!("Waiting for dummy byte (Video Socket)...");
            let mut dummy = [0u8; 1];
            match timeout(Self::READ_TIMEOUT, video_reader.read_exact(&mut dummy)).await {
                Ok(Ok(_)) => {
                    tracing::info!("Consuming dummy byte: 0x{:02X}", dummy[0]);
                }
                Ok(Err(e)) => return Err(anyhow::anyhow!("Failed to read dummy byte: {}", e)),
                Err(_) => return Err(NetworkError::Timeout.into()),
            }

            // Read Video Metadata
            tracing::info!("Waiting for video metadata (Video Socket)...");
            let mut v_meta = [0u8; 12];
            match timeout(Self::READ_TIMEOUT, video_reader.read_exact(&mut v_meta)).await {
                Ok(Ok(_)) => {
                    let v_codec_id = u32::from_be_bytes(v_meta[0..4].try_into().unwrap());
                    let width = u32::from_be_bytes(v_meta[4..8].try_into().unwrap());
                    let height = u32::from_be_bytes(v_meta[8..12].try_into().unwrap());
                    tracing::info!(
                        "Video: CodecID=0x{:08X}, W={}, H={}",
                        v_codec_id,
                        width,
                        height
                    );
                    Ok(())
                }
                Ok(Err(e)) => Err(anyhow::anyhow!("Failed to read video metadata: {}", e)),
                Err(_) => Err(NetworkError::Timeout.into()),
            }
        };

        let audio_metadata_future = async {
            if let Some(mut reader) = audio_reader {
                tracing::info!("Waiting for audio metadata (Audio Socket)...");
                let mut a_meta = [0u8; 4];
                match timeout(Self::READ_TIMEOUT, reader.read_exact(&mut a_meta)).await {
                    Ok(Ok(_)) => {
                        let a_codec_id = u32::from_be_bytes(a_meta);
                        tracing::info!("Audio: CodecID=0x{:08X}", a_codec_id);
                        if a_codec_id == 0 {
                            tracing::warn!("Audio disabled by server (CodecID=0).");
                            None // Disable audio
                        } else {
                            Some(reader) // Return reader
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Failed to read audio metadata: {}. Disabling audio.", e);
                        None
                    }
                    Err(_) => {
                        tracing::warn!("Timeout waiting for audio metadata. Disabling audio.");
                        None
                    }
                }
            } else {
                None
            }
        };

        // Run metadata reads concurrently
        let (video_res, audio_res) = tokio::join!(video_metadata_future, audio_metadata_future);

        video_res.map_err(|e| {
            NetworkError::ConnectionFailed(format!("Video metadata handshake failed: {}", e))
        })?;
        let audio_reader = audio_res;

        // 7. Spawn Readers
        let (tx, packet_rx) = tokio::sync::mpsc::channel(100);

        // Video Reader Task
        let tx_video = tx.clone();
        tokio::spawn(async move {
            loop {
                match Self::read_packet(&mut video_reader, PacketType::Video).await {
                    Ok(pkt) => {
                        if tx_video.send(Ok(pkt)).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx_video.send(Err(e)).await;
                        break;
                    }
                }
            }
        });

        // Audio Reader Task
        if let Some(mut reader) = audio_reader {
            let tx_audio = tx.clone();
            tokio::spawn(async move {
                loop {
                    match Self::read_packet(&mut reader, PacketType::Audio).await {
                        Ok(pkt) => {
                            if tx_audio.send(Ok(pkt)).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = tx_audio.send(Err(e)).await;
                            break;
                        }
                    }
                }
            });
        }

        Ok(Self {
            control_writer,
            packet_rx,
            stats: NetworkStats::default(),
        })
    }

    async fn recv(&mut self) -> Result<Packet> {
        match self.packet_rx.recv().await {
            Some(Ok(packet)) => {
                self.stats.bytes_received += packet.data.len() as u64;
                self.stats.packets_received += 1;
                Ok(packet)
            }
            Some(Err(e)) => Err(e.into()),
            None => Err(NetworkError::ConnectionClosed.into()),
        }
    }

    async fn send_control(&mut self, msg: ControlMessage) -> Result<()> {
        let data = msg
            .to_bytes()
            .map_err(|e| NetworkError::Protocol(e.to_string()))?;
        let packet = Packet::new(PacketType::Control, 0, 0, data);
        self.control_writer.write_all(&packet.to_bytes()).await?;
        self.control_writer.flush().await?;
        Ok(())
    }

    fn stats(&self) -> NetworkStats {
        self.stats
    }

    async fn close(&mut self) -> Result<()> {
        self.packet_rx.close(); // Stop receiving
        // Stream shutdown happens when dropped
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
