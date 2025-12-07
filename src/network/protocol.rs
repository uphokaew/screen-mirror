use bytes::{Buf, Bytes, BytesMut};
use serde::{Deserialize, Serialize};

/// Packet types in the protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PacketType {
    /// Video frame packet
    Video = 0x01,

    /// Audio frame packet
    Audio = 0x02,

    /// Control message
    Control = 0x03,

    /// FEC (Forward Error Correction) packet
    Fec = 0x04,

    /// Handshake/Capability negotiation
    Handshake = 0x05,
}

impl TryFrom<u8> for PacketType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(PacketType::Video),
            0x02 => Ok(PacketType::Audio),
            0x03 => Ok(PacketType::Control),
            0x04 => Ok(PacketType::Fec),
            0x05 => Ok(PacketType::Handshake),
            _ => Err(()),
        }
    }
}

/// Packet structure with PTS (Presentation Timestamp)
#[derive(Debug, Clone)]
pub struct Packet {
    /// Packet type
    pub packet_type: PacketType,

    /// Presentation timestamp in microseconds
    pub pts: i64,

    /// Sequence number for ordering/loss detection
    pub seq: u32,

    /// Payload data
    pub data: Bytes,
}

impl Packet {
    /// Packet header size: type(1) + pts(8) + seq(4) + len(4) = 17 bytes
    pub const HEADER_SIZE: usize = 17;

    /// Create a new packet
    pub fn new(packet_type: PacketType, pts: i64, seq: u32, data: Bytes) -> Self {
        Self {
            packet_type,
            pts,
            seq,
            data,
        }
    }

    /// Serialize packet to bytes (for sending)
    pub fn to_bytes(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(Self::HEADER_SIZE + self.data.len());

        // Write header
        buf.extend_from_slice(&[self.packet_type as u8]);
        buf.extend_from_slice(&self.pts.to_le_bytes());
        buf.extend_from_slice(&self.seq.to_le_bytes());
        buf.extend_from_slice(&(self.data.len() as u32).to_le_bytes());

        // Write payload
        buf.extend_from_slice(&self.data);

        buf
    }

    /// Deserialize packet from bytes (for receiving)
    pub fn from_bytes(mut buf: Bytes) -> Result<Self, &'static str> {
        if buf.len() < Self::HEADER_SIZE {
            return Err("Packet too short");
        }

        // Parse header
        let packet_type = PacketType::try_from(buf.get_u8()).map_err(|_| "Invalid packet type")?;

        let pts = buf.get_i64_le();
        let seq = buf.get_u32_le();
        let len = buf.get_u32_le() as usize;

        if buf.remaining() < len {
            return Err("Incomplete packet");
        }

        let data = buf.split_to(len);

        Ok(Self::new(packet_type, pts, seq, data))
    }

    /// Check if this is a video keyframe (I-frame)
    /// Detects H.264 NAL unit type 5 or H.265 NAL unit type 19/20
    pub fn is_keyframe(&self) -> bool {
        if self.packet_type != PacketType::Video {
            return false;
        }

        if self.data.len() < 5 {
            return false;
        }

        // Check for H.264 start code (00 00 00 01 or 00 00 01)
        let has_start_code = (self.data.len() >= 4 && &self.data[0..4] == &[0, 0, 0, 1])
            || (self.data.len() >= 3 && &self.data[0..3] == &[0, 0, 1]);

        if !has_start_code {
            return false;
        }

        // Find NAL unit header
        let nal_start = if &self.data[0..4] == &[0, 0, 0, 1] {
            4
        } else {
            3
        };
        if self.data.len() <= nal_start {
            return false;
        }

        let nal_header = self.data[nal_start];

        // H.264: NAL unit type 5 (IDR)
        let h264_idr = (nal_header & 0x1F) == 5;

        // H.265: NAL unit type 19 (IDR_W_RADL) or 20 (IDR_N_LP)
        let h265_idr = {
            let nal_type = (nal_header >> 1) & 0x3F;
            nal_type == 19 || nal_type == 20
        };

        h264_idr || h265_idr
    }
}

/// Control messages sent between client and server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMessage {
    /// Set video bitrate (Mbps)
    SetBitrate(u32),

    /// Set video resolution
    SetResolution { width: u32, height: u32 },

    /// Set frame rate
    SetFrameRate(u32),

    /// Request keyframe
    RequestKeyframe,

    /// Capability announcement from server
    Capabilities {
        max_resolution: (u32, u32),
        codecs: Vec<String>,
        audio_supported: bool,
    },

    /// Acknowledge receipt
    Ack { seq: u32 },
}

impl ControlMessage {
    /// Serialize to bytes using bincode
    pub fn to_bytes(&self) -> Result<Bytes, bincode::Error> {
        let data = bincode::serialize(self)?;
        Ok(Bytes::from(data))
    }

    /// Deserialize from bytes using bincode
    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

/// FEC (Forward Error Correction) packet
/// Uses Reed-Solomon erasure coding for packet recovery
#[derive(Debug, Clone)]
pub struct FecPacket {
    /// Block ID (identifies which group of packets this FEC belongs to)
    pub block_id: u32,

    /// Index within the FEC block
    pub index: u8,

    /// Total number of data packets in this block
    pub data_count: u8,

    /// Total number of parity packets in this block
    pub parity_count: u8,

    /// FEC data
    pub data: Bytes,
}

impl FecPacket {
    /// FEC header size: block_id(4) + index(1) + data_count(1) + parity_count(1) = 7 bytes
    pub const HEADER_SIZE: usize = 7;

    /// Create new FEC packet
    pub fn new(block_id: u32, index: u8, data_count: u8, parity_count: u8, data: Bytes) -> Self {
        Self {
            block_id,
            index,
            data_count,
            parity_count,
            data,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(Self::HEADER_SIZE + self.data.len());
        buf.extend_from_slice(&self.block_id.to_le_bytes());
        buf.extend_from_slice(&[self.index, self.data_count, self.parity_count]);
        buf.extend_from_slice(&self.data);
        buf
    }

    /// Deserialize from bytes
    pub fn from_bytes(mut buf: Bytes) -> Result<Self, &'static str> {
        if buf.len() < Self::HEADER_SIZE {
            return Err("FEC packet too short");
        }

        let block_id = buf.get_u32_le();
        let index = buf.get_u8();
        let data_count = buf.get_u8();
        let parity_count = buf.get_u8();
        let data = buf;

        Ok(Self::new(block_id, index, data_count, parity_count, data))
    }
}
