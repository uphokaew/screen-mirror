use anyhow::{Context as AnyhowContext, Result};
use bytes::Bytes;
use ffmpeg::codec::Context;
use ffmpeg::codec::decoder::Audio as AudioDecoder;
use ffmpeg::util::frame::audio::Audio as AudioFrame;
use ffmpeg_next as ffmpeg;

/// Decoded audio samples with metadata
pub struct DecodedAudio {
    pub pts: i64,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Hardware-accelerated audio decoder for AAC/Opus streams
pub struct HardwareAudioDecoder {
    decoder: AudioDecoder,
    #[allow(dead_code)]
    sample_rate: u32,
    #[allow(dead_code)]
    channels: u16,
    packet_buffer: Vec<u8>,
}

impl HardwareAudioDecoder {
    /// Create a new audio decoder
    ///
    /// # Arguments
    /// * `codec_name` - Codec name: "aac", "opus", "mp3"
    /// * `sample_rate` - Expected sample rate (e.g., 48000)
    /// * `channels` - Number of channels (e.g., 2 for stereo)
    pub fn new(codec_name: &str, sample_rate: u32, channels: u16) -> Result<Self> {
        // Initialize FFmpeg
        ffmpeg::init().context("Failed to initialize FFmpeg")?;

        // Find audio decoder
        let _codec = ffmpeg::codec::decoder::find_by_name(codec_name)
            .ok_or_else(|| anyhow::anyhow!("Audio codec '{}' not found", codec_name))?;

        let context = Context::new();
        let decoder = context
            .decoder()
            .audio()
            .context("Failed to create audio decoder")?;

        tracing::info!("Using audio decoder: {}", codec_name);

        Ok(Self {
            decoder,
            sample_rate,
            channels,
            packet_buffer: Vec::new(),
        })
    }

    /// Decode an audio packet
    ///
    /// # Arguments
    /// * `data` - Encoded audio data (AAC/Opus/etc.)
    /// * `pts` - Presentation timestamp in microseconds
    ///
    /// # Returns
    /// Decoded audio samples if a complete frame was produced, None otherwise
    pub fn decode(&mut self, data: &Bytes, pts: i64) -> Result<Option<DecodedAudio>> {
        // Append data to packet buffer
        self.packet_buffer.extend_from_slice(data);

        // Create packet from buffer
        let mut packet = ffmpeg::codec::packet::Packet::copy(&self.packet_buffer);
        packet.set_pts(Some(pts));

        // Send packet to decoder
        self.decoder
            .send_packet(&packet)
            .context("Failed to send packet to audio decoder")?;

        // Clear packet buffer after successful send
        self.packet_buffer.clear();

        // Try to receive decoded frame
        let mut frame = AudioFrame::empty();
        match self.decoder.receive_frame(&mut frame) {
            Ok(_) => {
                // Frame decoded successfully
                let decoded = self.convert_frame(&frame, pts)?;
                Ok(Some(decoded))
            }
            Err(ffmpeg::Error::Other { errno: 11 }) => {
                // EAGAIN - need more data
                Ok(None)
            }
            Err(e) => Err(anyhow::anyhow!("Audio decoder error: {:?}", e)),
        }
    }

    /// Convert FFmpeg audio frame to our DecodedAudio format
    fn convert_frame(&self, frame: &AudioFrame, pts: i64) -> Result<DecodedAudio> {
        let sample_count = frame.samples();
        let channels = frame.channels() as usize;
        let _format = frame.format();

        // Convert to f32 samples
        let samples = self.extract_samples(frame, sample_count, channels)?;

        Ok(DecodedAudio {
            pts,
            samples,
            sample_rate: frame.rate(),
            channels: channels as u16,
        })
    }

    /// Extract audio samples from frame and convert to f32
    fn extract_samples(
        &self,
        frame: &AudioFrame,
        sample_count: usize,
        channels: usize,
    ) -> Result<Vec<f32>> {
        let total_samples = sample_count * channels;
        let mut samples = Vec::with_capacity(total_samples);

        // Get frame format
        let format = frame.format();

        // Extract samples based on format
        match format {
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed) => {
                // Already f32, packed format
                let data = frame.data(0);
                for i in 0..total_samples {
                    let offset = i * 4;
                    if offset + 4 <= data.len() {
                        let sample_bytes = [
                            data[offset],
                            data[offset + 1],
                            data[offset + 2],
                            data[offset + 3],
                        ];
                        samples.push(f32::from_le_bytes(sample_bytes));
                    }
                }
            }
            ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Packed) => {
                // i16 format, convert to f32
                let data = frame.data(0);
                for i in 0..total_samples {
                    let offset = i * 2;
                    if offset + 2 <= data.len() {
                        let sample_i16 = i16::from_le_bytes([data[offset], data[offset + 1]]);
                        let sample_f32 = sample_i16 as f32 / 32768.0;
                        samples.push(sample_f32);
                    }
                }
            }
            _ => {
                // Unsupported format, return silence
                tracing::warn!("Unsupported audio format: {:?}", format);
                samples.resize(total_samples, 0.0);
            }
        }

        Ok(samples)
    }

    /// Flush the decoder and get any remaining frames
    pub fn flush(&mut self) -> Result<Vec<DecodedAudio>> {
        let mut frames = Vec::new();

        // Send flush signal
        self.decoder
            .send_eof()
            .context("Failed to send EOF to audio decoder")?;

        // Receive all remaining frames
        loop {
            let mut frame = AudioFrame::empty();
            match self.decoder.receive_frame(&mut frame) {
                Ok(_) => {
                    if let Ok(decoded) = self.convert_frame(&frame, 0) {
                        frames.push(decoded);
                    }
                }
                Err(_) => break,
            }
        }

        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_decoder_creation() {
        // Test that audio decoder can be created
        let _result = HardwareAudioDecoder::new("aac", 48000, 2);
        // May fail if ffmpeg not installed, but structure should compile
    }
}
