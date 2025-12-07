use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Connection configuration
    pub connection: ConnectionConfig,

    /// Video configuration
    pub video: VideoConfig,

    /// Audio configuration
    pub audio: AudioConfig,

    /// Performance tuning
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Connection mode (TCP or QUIC)
    pub mode: ConnectionMode,

    /// Server IP address
    pub host: IpAddr,

    /// Server port
    pub port: u16,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionMode {
    Tcp,
    Quic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoConfig {
    /// Target resolution (upscaling will be applied if monitor is larger)
    pub resolution: Resolution,

    /// Video codec
    pub codec: VideoCodec,

    /// Initial bitrate (Mbps)
    pub bitrate: u32,

    /// Enable hardware decoding
    pub hw_accel: bool,

    /// Hardware decoder preference (nvdec, qsv, vaapi, auto)
    pub hw_decoder: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Resolution {
    HD720,   // 1280x720
    FHD1080, // 1920x1080
    QHD1440, // 2560x1440
}

impl Resolution {
    pub fn width(&self) -> u32 {
        match self {
            Resolution::HD720 => 1280,
            Resolution::FHD1080 => 1920,
            Resolution::QHD1440 => 2560,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            Resolution::HD720 => 720,
            Resolution::FHD1080 => 1080,
            Resolution::QHD1440 => 1440,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodec {
    H264,
    H265,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Enable audio streaming
    pub enabled: bool,

    /// Sample rate (Hz)
    pub sample_rate: u32,

    /// Number of channels
    pub channels: u16,

    /// Audio codec
    pub codec: AudioCodec,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    Aac,
    Opus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Video frame buffer size
    pub video_buffer_size: usize,

    /// Audio buffer size
    pub audio_buffer_size: usize,

    /// Jitter buffer size (ms) for wireless
    pub jitter_buffer_ms: u32,

    /// Enable adaptive bitrate
    pub adaptive_bitrate: bool,

    /// FEC redundancy percentage (0-50)
    pub fec_redundancy: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig {
                mode: ConnectionMode::Tcp,
                host: "127.0.0.1".parse().unwrap(),
                port: 5555,
            },
            video: VideoConfig {
                resolution: Resolution::FHD1080,
                codec: VideoCodec::H264,
                bitrate: 8,
                hw_accel: true,
                hw_decoder: "auto".to_string(),
            },
            audio: AudioConfig {
                enabled: true,
                sample_rate: 48000,
                channels: 2,
                codec: AudioCodec::Aac,
            },
            performance: PerformanceConfig {
                video_buffer_size: 16,
                audio_buffer_size: 64,
                jitter_buffer_ms: 30,
                adaptive_bitrate: true,
                fec_redundancy: 10,
            },
        }
    }
}
