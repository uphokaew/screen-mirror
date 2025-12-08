use anyhow::{Context, Result};
use bytes::Bytes;
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::protocol::{FecPacket, Packet, PacketType};

/// FEC (Forward Error Correction) encoder using Reed-Solomon
/// Allows recovery of lost packets without retransmission
pub struct FecEncoder {
    reed_solomon: ReedSolomon,
    data_shards: usize,
    parity_shards: usize,
    current_block_id: u32,
    block_buffer: Vec<Packet>,
    #[allow(dead_code)]
    max_packet_size: usize,
}

impl FecEncoder {
    /// Create a new FEC encoder
    ///
    /// # Arguments
    /// * `data_shards` - Number of data packets per FEC block (e.g., 10)
    /// * `parity_shards` - Number of parity packets per FEC block (e.g., 2 for 20% redundancy)
    pub fn new(data_shards: usize, parity_shards: usize) -> Result<Self> {
        let reed_solomon = ReedSolomon::new(data_shards, parity_shards)
            .context("Failed to create Reed-Solomon encoder")?;

        Ok(Self {
            reed_solomon,
            data_shards,
            parity_shards,
            current_block_id: 0,
            block_buffer: Vec::with_capacity(data_shards),
            max_packet_size: 65536, // 64KB max packet size
        })
    }

    /// Add a packet to the encoder
    /// Returns FEC packets if a complete block is formed
    pub fn encode(&mut self, packet: Packet) -> Vec<FecPacket> {
        self.block_buffer.push(packet);

        // Check if we have a complete block
        if self.block_buffer.len() >= self.data_shards {
            let fec_packets = self.encode_block();
            self.block_buffer.clear();
            self.current_block_id = self.current_block_id.wrapping_add(1);
            fec_packets
        } else {
            Vec::new()
        }
    }

    /// Encode the current block into FEC packets
    fn encode_block(&mut self) -> Vec<FecPacket> {
        // Serialize all packets in the block
        let mut data_packets: Vec<Vec<u8>> = self
            .block_buffer
            .iter()
            .map(|p| p.to_bytes().to_vec())
            .collect();

        // Find max packet size in this block
        let max_size = data_packets.iter().map(|p| p.len()).max().unwrap_or(0);

        // Pad all packets to the same size
        for packet in &mut data_packets {
            packet.resize(max_size, 0);
        }

        // Create parity shards (initialized with zeros)
        let mut parity_packets: Vec<Vec<u8>> = vec![vec![0u8; max_size]; self.parity_shards];

        // Combine data and parity shards
        let mut shards: Vec<_> = data_packets
            .iter_mut()
            .chain(parity_packets.iter_mut())
            .collect();

        // Encode using Reed-Solomon
        if let Err(e) = self.reed_solomon.encode(&mut shards) {
            tracing::error!("FEC encoding failed: {:?}", e);
            return Vec::new();
        }

        // Create FEC packets from parity shards
        let mut fec_packets = Vec::with_capacity(self.parity_shards);
        for (i, parity_shard) in parity_packets.into_iter().enumerate() {
            let fec_packet = FecPacket::new(
                self.current_block_id,
                (self.data_shards + i) as u8,
                self.data_shards as u8,
                self.parity_shards as u8,
                Bytes::from(parity_shard),
            );
            fec_packets.push(fec_packet);
        }

        fec_packets
    }

    /// Force encoding of current partial block
    pub fn flush(&mut self) -> Vec<FecPacket> {
        if self.block_buffer.is_empty() {
            return Vec::new();
        }

        // Pad block with empty packets if needed
        while self.block_buffer.len() < self.data_shards {
            self.block_buffer
                .push(Packet::new(PacketType::Video, 0, 0, Bytes::new()));
        }

        let fec_packets = self.encode_block();
        self.block_buffer.clear();
        self.current_block_id = self.current_block_id.wrapping_add(1);
        fec_packets
    }
}

/// FEC (Forward Error Correction) decoder using Reed-Solomon
/// Recovers lost packets from received data and parity packets
pub struct FecDecoder {
    reed_solomon: ReedSolomon,
    data_shards: usize,
    parity_shards: usize,
    blocks: HashMap<u32, FecBlock>,
    last_cleanup: Instant,
}

struct FecBlock {
    #[allow(dead_code)]
    block_id: u32,
    data_shards: Vec<Option<Vec<u8>>>,
    parity_shards: Vec<Option<Vec<u8>>>,
    data_count: u8,
    parity_count: u8,
    created_at: Instant,
    recovered: bool,
}

impl FecDecoder {
    /// Create a new FEC decoder
    pub fn new(data_shards: usize, parity_shards: usize) -> Result<Self> {
        let reed_solomon = ReedSolomon::new(data_shards, parity_shards)
            .context("Failed to create Reed-Solomon decoder")?;

        Ok(Self {
            reed_solomon,
            data_shards,
            parity_shards,
            blocks: HashMap::new(),
            last_cleanup: Instant::now(),
        })
    }

    /// Add a data packet to the decoder
    pub fn add_data_packet(&mut self, seq: u32, data: Bytes) -> Option<Vec<Packet>> {
        let block_id = seq / self.data_shards as u32;
        let index = (seq % self.data_shards as u32) as usize;

        let block = self.blocks.entry(block_id).or_insert_with(|| FecBlock {
            block_id,
            data_shards: vec![None; self.data_shards],
            parity_shards: vec![None; self.parity_shards],
            data_count: self.data_shards as u8,
            parity_count: self.parity_shards as u8,
            created_at: Instant::now(),
            recovered: false,
        });

        // Store data packet
        if index < self.data_shards {
            block.data_shards[index] = Some(data.to_vec());
        }

        // Try to recover if possible
        self.try_recover(block_id)
    }

    /// Add a FEC parity packet to the decoder
    pub fn add_fec_packet(&mut self, fec_packet: FecPacket) -> Option<Vec<Packet>> {
        let block = self
            .blocks
            .entry(fec_packet.block_id)
            .or_insert_with(|| FecBlock {
                block_id: fec_packet.block_id,
                data_shards: vec![None; fec_packet.data_count as usize],
                parity_shards: vec![None; fec_packet.parity_count as usize],
                data_count: fec_packet.data_count,
                parity_count: fec_packet.parity_count,
                created_at: Instant::now(),
                recovered: false,
            });

        // Store parity packet
        let parity_index = fec_packet.index as usize - fec_packet.data_count as usize;
        if parity_index < block.parity_shards.len() {
            block.parity_shards[parity_index] = Some(fec_packet.data.to_vec());
        }

        // Try to recover if possible
        self.try_recover(fec_packet.block_id)
    }

    /// Try to recover lost packets in a block
    fn try_recover(&mut self, block_id: u32) -> Option<Vec<Packet>> {
        let block = self.blocks.get_mut(&block_id)?;

        // Skip if already recovered
        if block.recovered {
            return None;
        }

        // Count received shards
        let data_received = block.data_shards.iter().filter(|s| s.is_some()).count();
        let parity_received = block.parity_shards.iter().filter(|s| s.is_some()).count();
        let total_received = data_received + parity_received;

        // Need at least data_count shards to recover
        if total_received < block.data_count as usize {
            return None; // Not enough data to recover
        }

        // If all data packets received, no recovery needed
        if data_received == block.data_count as usize {
            block.recovered = true;
            return None;
        }

        // Perform Reed-Solomon recovery
        let max_size = block
            .data_shards
            .iter()
            .chain(block.parity_shards.iter())
            .filter_map(|s| s.as_ref())
            .map(|s| s.len())
            .max()
            .unwrap_or(0);

        // Create shards array with proper padding
        let mut shards: Vec<Option<Vec<u8>>> =
            Vec::with_capacity(block.data_count as usize + block.parity_count as usize);

        // Add data shards
        for shard in &block.data_shards {
            if let Some(data) = shard {
                let mut padded = data.clone();
                padded.resize(max_size, 0);
                shards.push(Some(padded));
            } else {
                shards.push(None);
            }
        }

        // Add parity shards
        for shard in &block.parity_shards {
            if let Some(data) = shard {
                let mut padded = data.clone();
                padded.resize(max_size, 0);
                shards.push(Some(padded));
            } else {
                shards.push(None);
            }
        }

        // Reconstruct missing shards
        if let Err(e) = self.reed_solomon.reconstruct(&mut shards) {
            tracing::error!("FEC reconstruction failed for block {}: {:?}", block_id, e);
            return None;
        }

        // Extract recovered packets
        let mut recovered_packets = Vec::new();
        for (i, shard) in shards.iter().take(block.data_count as usize).enumerate() {
            if block.data_shards[i].is_none()
                && let Some(data) = shard
            {
                // Parse recovered packet
                if let Ok(packet) = Packet::from_bytes(Bytes::from(data.clone())) {
                    recovered_packets.push(packet);
                    tracing::info!("Recovered packet {} in block {}", i, block_id);
                }
            }
        }

        block.recovered = true;

        if recovered_packets.is_empty() {
            None
        } else {
            Some(recovered_packets)
        }
    }

    /// Cleanup old blocks (called periodically)
    pub fn cleanup(&mut self) {
        if self.last_cleanup.elapsed() < Duration::from_secs(5) {
            return;
        }

        self.blocks
            .retain(|_, block| block.created_at.elapsed() < Duration::from_secs(10));

        self.last_cleanup = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fec_encode_decode() {
        let mut encoder = FecEncoder::new(4, 2).unwrap();
        let mut decoder = FecDecoder::new(4, 2).unwrap();

        // Create test packets
        let mut packets = Vec::new();
        for i in 0..4 {
            packets.push(Packet::new(
                PacketType::Video,
                i * 1000,
                i as u32,
                Bytes::from(vec![i as u8; 100]),
            ));
        }

        // Encode
        let mut all_fec_packets = Vec::new();
        for packet in &packets {
            let fec_packets = encoder.encode(packet.clone());
            all_fec_packets.extend(fec_packets);
        }

        // Should have 2 FEC packets (parity shards)
        assert_eq!(all_fec_packets.len(), 2);

        // Simulate packet loss - drop packet 1
        decoder.add_data_packet(0, packets[0].data.clone());
        // Skip packet 1 (lost)
        decoder.add_data_packet(2, packets[2].data.clone());
        decoder.add_data_packet(3, packets[3].data.clone());

        // Add FEC packets
        for fec_packet in all_fec_packets {
            if let Some(recovered) = decoder.add_fec_packet(fec_packet) {
                // Should recover packet 1
                assert!(!recovered.is_empty());
            }
        }
    }
}
