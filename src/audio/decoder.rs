use anyhow::{anyhow, Result};
use audiopus::{coder::Decoder as OpusDecoder, Channels, SampleRate as OpusSampleRate};
use bytes::Bytes;
use symphonia::core::audio::AudioBufferRef;
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, DecoderOptions, CODEC_TYPE_NULL};

/// Decoded audio samples with metadata
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub pts: i64,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub enum AudioBackend {
    Opus(OpusWrapper),
    Symphonia(SymphoniaWrapper),
}

/// Smart Audio Decoder that selects the best backend
pub struct HardwareAudioDecoder {
    backend: AudioBackend,
    _sample_rate: u32,
    _channels: u16,
}

impl HardwareAudioDecoder {
    pub fn new(codec_name: &str, sample_rate: u32, channels: u16) -> Result<Self> {
        let backend = match codec_name.to_lowercase().as_str() {
            "opus" => {
                tracing::info!("Initializing specialized Opus decoder");
                AudioBackend::Opus(OpusWrapper::new(sample_rate, channels)?)
            }
            "aac" | "mp3" | "flac" | "wav" => {
                tracing::info!("Initializing Symphonia decoder for {}", codec_name);
                AudioBackend::Symphonia(SymphoniaWrapper::new(codec_name, sample_rate, channels)?)
            }
            _ => return Err(anyhow!("Unsupported codec: {}", codec_name)),
        };

        Ok(Self {
            backend,
            _sample_rate: sample_rate,
            _channels: channels,
        })
    }

    pub fn decode(&mut self, data: &Bytes, pts: i64) -> Result<Option<DecodedAudio>> {
        match &mut self.backend {
            AudioBackend::Opus(decoder) => decoder.decode(data, pts),
            AudioBackend::Symphonia(decoder) => decoder.decode(data, pts),
        }
    }
}

pub struct OpusWrapper {
    decoder: OpusDecoder,
    channels: audiopus::Channels,
    sample_rate: u32,
}

impl OpusWrapper {
    pub fn new(sample_rate: u32, channels: u16) -> Result<Self> {
        let opus_channels = match channels {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            _ => return Err(anyhow!("Opus only supports 1 or 2 channels")),
        };

        let opus_rate = match sample_rate {
            48000 => OpusSampleRate::Hz48000,
            24000 => OpusSampleRate::Hz24000,
            16000 => OpusSampleRate::Hz16000,
            12000 => OpusSampleRate::Hz12000,
            8000 => OpusSampleRate::Hz8000,
            _ => return Err(anyhow!("Unsupported Opus sample rate: {}", sample_rate)),
        };

        let decoder = OpusDecoder::new(opus_rate, opus_channels)?;

        Ok(Self {
            decoder,
            channels: opus_channels,
            sample_rate,
        })
    }

    pub fn decode(&mut self, data: &Bytes, pts: i64) -> Result<Option<DecodedAudio>> {
        let mut out = vec![0.0f32; 5760 * self.channels as usize];
        let input: &[u8] = data;
        match self.decoder.decode_float(Some(input), &mut out, false) {
            Ok(samples_decoded) => {
                out.truncate(samples_decoded * self.channels as usize);
                Ok(Some(DecodedAudio {
                    pts,
                    samples: out,
                    sample_rate: self.sample_rate,
                    channels: self.channels as u16,
                }))
            }
            Err(e) => Err(anyhow!("Opus decode error: {:?}", e)),
        }
    }
}

pub struct SymphoniaWrapper {
    decoder: Box<dyn SymphoniaDecoder>,
    sample_rate: u32,
    channels: u16,
}

impl SymphoniaWrapper {
    pub fn new(codec_name: &str, sample_rate: u32, channels: u16) -> Result<Self> {
        let codec_registry = symphonia::default::get_codecs();

        let hint = match codec_name {
            "aac" => symphonia::core::codecs::CODEC_TYPE_AAC,
            "mp3" => symphonia::core::codecs::CODEC_TYPE_MP3,
            "flac" => symphonia::core::codecs::CODEC_TYPE_FLAC,
            "pcm" | "raw" => symphonia::core::codecs::CODEC_TYPE_PCM_S16LE,
            _ => CODEC_TYPE_NULL,
        };

        if hint == CODEC_TYPE_NULL {
            return Err(anyhow!("Unknown codec for Symphonia: {}", codec_name));
        }

        let _codec = codec_registry
            .get_codec(hint)
            .ok_or_else(|| anyhow!("Codec not found in Symphonia registry"))?;

        let decoder = codec_registry.make(
            &symphonia::core::codecs::CodecParameters {
                codec: hint,
                sample_rate: Some(sample_rate),
                ..Default::default()
            },
            &DecoderOptions::default(),
        )?;

        Ok(Self {
            decoder,
            sample_rate,
            channels,
        })
    }

    pub fn decode(&mut self, data: &Bytes, pts: i64) -> Result<Option<DecodedAudio>> {
        let packet = symphonia::core::formats::Packet::new_from_slice(0, 0, 0, data);

        match self.decoder.decode(&packet) {
            Ok(decoded) => {
                let samples = Self::convert_buffer(&decoded);
                Ok(Some(DecodedAudio {
                    pts,
                    samples,
                    sample_rate: self.sample_rate,
                    channels: self.channels,
                }))
            }
            Err(e) => Err(anyhow!("Symphonia decode error: {}", e)),
        }
    }

    fn convert_buffer(decoded: &AudioBufferRef) -> Vec<f32> {
        use symphonia::core::audio::Signal;
        use symphonia::core::conv::FromSample;

        let mut samples = Vec::new();

        match decoded {
            symphonia::core::audio::AudioBufferRef::F32(buf) => {
                for i in 0..buf.frames() {
                    for c in 0..buf.spec().channels.count() {
                        samples.push(buf.chan(c)[i]);
                    }
                }
            }
            symphonia::core::audio::AudioBufferRef::S16(buf) => {
                for i in 0..buf.frames() {
                    for c in 0..buf.spec().channels.count() {
                        samples.push(f32::from_sample(buf.chan(c)[i]));
                    }
                }
            }
            symphonia::core::audio::AudioBufferRef::U8(buf) => {
                for i in 0..buf.frames() {
                    for c in 0..buf.spec().channels.count() {
                        samples.push(f32::from_sample(buf.chan(c)[i]));
                    }
                }
            }
            _ => tracing::warn!("Unsupported sample format from Symphonia"),
        }
        samples
    }
}

use symphonia;
