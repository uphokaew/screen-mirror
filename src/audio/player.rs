use crate::audio::decoder::DecodedAudio;
use anyhow::{Context, Result};
use cpal::{
    Device, SampleRate, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Audio player with jitter buffer for wireless connections
pub struct AudioPlayer {
    _device: Device,
    _stream: Stream,
    jitter_buffer: Arc<Mutex<JitterBuffer>>,
    volume: f32,
}

/// Jitter buffer for handling packet reordering and timing jitter
struct JitterBuffer {
    buffer: VecDeque<DecodedAudio>,
    #[allow(dead_code)]
    max_size_ms: u32,
    current_size_samples: usize,
    max_size_samples: usize,
    #[allow(dead_code)]
    sample_rate: u32,
    #[allow(dead_code)]
    channels: u16,
}

impl JitterBuffer {
    fn new(max_size_ms: u32, sample_rate: u32, channels: u16) -> Self {
        let max_size_samples =
            (max_size_ms as usize * sample_rate as usize / 1000) * channels as usize;

        Self {
            buffer: VecDeque::new(),
            max_size_ms,
            current_size_samples: 0,
            max_size_samples,
            sample_rate,
            channels,
        }
    }

    fn push(&mut self, audio: DecodedAudio) {
        self.current_size_samples += audio.samples.len();
        self.buffer.push_back(audio);

        // Trim buffer if too large
        while self.current_size_samples > self.max_size_samples && !self.buffer.is_empty() {
            if let Some(old_audio) = self.buffer.pop_front() {
                self.current_size_samples -= old_audio.samples.len();
            }
        }
    }

    fn pop_samples(&mut self, count: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(count);

        while samples.len() < count {
            if let Some(audio) = self.buffer.front_mut() {
                let remaining = count - samples.len();
                let available = audio.samples.len().min(remaining);

                // Drain samples from front of audio buffer
                for _ in 0..available {
                    if !audio.samples.is_empty() {
                        samples.push(audio.samples.remove(0));
                    }
                }

                // Remove audio if all samples consumed
                if audio.samples.is_empty() {
                    self.buffer.pop_front();
                }
            } else {
                // No more audio in buffer, pad with silence
                samples.resize(count, 0.0);
                break;
            }
        }

        self.current_size_samples = self.current_size_samples.saturating_sub(samples.len());
        samples
    }

    fn underrun_risk(&self) -> bool {
        // Risk of underrun if buffer is less than 25% full
        self.current_size_samples < (self.max_size_samples / 4)
    }
}

impl AudioPlayer {
    /// Create a new audio player
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sample rate (e.g., 48000)
    /// * `channels` - Number of channels (1 = mono, 2 = stereo)
    /// * `jitter_buffer_ms` - Jitter buffer size in milliseconds (e.g., 30ms for wireless)
    pub fn new(sample_rate: u32, channels: u16, jitter_buffer_ms: u32) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("No audio output device available")?;

        tracing::info!(
            "Using audio device: {}",
            device.name().unwrap_or("Unknown".to_string())
        );

        let config = StreamConfig {
            channels,
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let jitter_buffer = Arc::new(Mutex::new(JitterBuffer::new(
            jitter_buffer_ms,
            sample_rate,
            channels,
        )));

        let jitter_buffer_clone = jitter_buffer.clone();

        // Create audio output stream
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut buffer = jitter_buffer_clone.lock().unwrap();
                    let samples = buffer.pop_samples(data.len());

                    // Copy samples to output
                    for (i, sample) in samples.iter().enumerate() {
                        if i < data.len() {
                            data[i] = *sample;
                        }
                    }

                    // Warn on underrun
                    if buffer.underrun_risk() {
                        tracing::warn!("Audio buffer underrun risk");
                    }
                },
                |err| {
                    tracing::error!("Audio stream error: {}", err);
                },
                None,
            )
            .context("Failed to build audio output stream")?;

        // Start the stream
        stream.play().context("Failed to start audio stream")?;

        Ok(Self {
            _device: device,
            _stream: stream,
            jitter_buffer,
            volume: 1.0,
        })
    }

    /// Queue audio for playback
    ///
    /// Audio will be added to the jitter buffer and played asynchronously
    pub fn play(&mut self, mut audio: DecodedAudio) -> Result<()> {
        // Apply volume
        if self.volume != 1.0 {
            for sample in &mut audio.samples {
                *sample *= self.volume;
            }
        }

        // Add to jitter buffer
        let mut buffer = self
            .jitter_buffer
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock jitter buffer: {}", e))?;

        buffer.push(audio);

        Ok(())
    }

    /// Set playback volume (0.0 - 1.0)
    pub fn set_volume(&mut self, volume: f32) -> Result<()> {
        self.volume = volume.clamp(0.0, 1.0);
        Ok(())
    }

    /// Get current buffer fill level (0.0 - 1.0)
    pub fn buffer_level(&self) -> f32 {
        if let Ok(buffer) = self.jitter_buffer.lock() {
            buffer.current_size_samples as f32 / buffer.max_size_samples as f32
        } else {
            0.0
        }
    }

    /// Check if buffer is at risk of underrun
    pub fn underrun_risk(&self) -> bool {
        if let Ok(buffer) = self.jitter_buffer.lock() {
            buffer.underrun_risk()
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jitter_buffer() {
        let mut buffer = JitterBuffer::new(30, 48000, 2);

        let audio = DecodedAudio {
            pts: 0,
            samples: vec![0.0; 1000],
            sample_rate: 48000,
            channels: 2,
        };

        buffer.push(audio);
        assert_eq!(buffer.current_size_samples, 1000);

        let samples = buffer.pop_samples(500);
        assert_eq!(samples.len(), 500);
        assert_eq!(buffer.current_size_samples, 500);
    }
}
