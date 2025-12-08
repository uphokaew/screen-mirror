/// Audio/Video synchronization engine using PTS
use std::collections::VecDeque;
use std::time::Instant;

/// Timestamped video frame
pub struct TimestampedFrame {
    pub pts: i64,
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Timestamped audio samples
pub struct TimestampedAudio {
    pub pts: i64,
    pub samples: Vec<f32>,
}

/// Sync action to take based on buffer states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    /// Continue normal playback
    Continue,

    /// Drop current video frame (video is ahead)
    DropVideoFrame,

    /// Skip audio samples (audio is ahead)
    SkipAudioSamples,

    /// Wait for more audio (audio buffer empty)
    WaitForAudio,

    /// Wait for more video (video buffer empty)
    WaitForVideo,
}

/// PTS-based audio/video synchronization engine
pub struct SyncEngine {
    video_buffer: VecDeque<TimestampedFrame>,
    audio_buffer: VecDeque<TimestampedAudio>,
    sync_threshold_ms: i64,
    max_video_buffer: usize,
    max_audio_buffer: usize,
    video_drift_ms: i64,
    audio_drift_ms: i64,
    #[allow(dead_code)]
    last_sync_check: Instant,
    stats: SyncStats,
}

/// Synchronization statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct SyncStats {
    pub video_frames_dropped: u64,
    pub audio_samples_skipped: u64,
    pub sync_corrections: u64,
    pub current_drift_ms: i64,
    pub avg_drift_ms: f64,
}

impl SyncEngine {
    /// Create a new synchronization engine
    ///
    /// # Arguments
    /// * `sync_threshold_ms` - Maximum acceptable A/V drift before correction (e.g., 50ms)
    /// * `max_video_buffer` - Maximum video frames to buffer
    /// * `max_audio_buffer` - Maximum audio chunks to buffer
    pub fn new(sync_threshold_ms: i64, max_video_buffer: usize, max_audio_buffer: usize) -> Self {
        Self {
            video_buffer: VecDeque::with_capacity(max_video_buffer),
            audio_buffer: VecDeque::with_capacity(max_audio_buffer),
            sync_threshold_ms,
            max_video_buffer,
            max_audio_buffer,
            video_drift_ms: 0,
            audio_drift_ms: 0,
            last_sync_check: Instant::now(),
            stats: SyncStats::default(),
        }
    }

    /// Add a video frame to the buffer
    pub fn add_video_frame(&mut self, pts: i64, data: Vec<u8>, width: u32, height: u32) {
        // Trim buffer if full
        while self.video_buffer.len() >= self.max_video_buffer {
            self.video_buffer.pop_front();
            self.stats.video_frames_dropped += 1;
        }

        self.video_buffer.push_back(TimestampedFrame {
            pts,
            data,
            width,
            height,
        });
    }

    /// Add audio samples to the buffer
    pub fn add_audio_samples(&mut self, pts: i64, samples: Vec<f32>) {
        // Trim buffer if full
        while self.audio_buffer.len() >= self.max_audio_buffer {
            self.audio_buffer.pop_front();
            self.stats.audio_samples_skipped += 1;
        }

        self.audio_buffer
            .push_back(TimestampedAudio { pts, samples });
    }

    /// Perform synchronization check and return action
    pub fn sync(&mut self) -> SyncAction {
        // Check buffer status
        if self.video_buffer.is_empty() {
            return SyncAction::WaitForVideo;
        }

        if self.audio_buffer.is_empty() {
            return SyncAction::WaitForAudio;
        }

        // Get current PTS for video and audio
        let video_pts = self.video_buffer.front().unwrap().pts;
        let audio_pts = self.audio_buffer.front().unwrap().pts;

        // Calculate drift (positive = video ahead, negative = audio ahead)
        let drift_us = video_pts - audio_pts;
        let drift_ms = drift_us / 1000;

        self.stats.current_drift_ms = drift_ms;

        // Update average drift
        self.stats.avg_drift_ms = self.stats.avg_drift_ms * 0.9 + drift_ms as f64 * 0.1;

        // Check if drift exceeds threshold
        if drift_ms.abs() > self.sync_threshold_ms {
            self.stats.sync_corrections += 1;

            if drift_ms > 0 {
                // Video is ahead of audio - drop video frame
                tracing::debug!("Video ahead by {}ms, dropping frame", drift_ms);
                self.stats.video_frames_dropped += 1;
                return SyncAction::DropVideoFrame;
            } else {
                // Audio is ahead of video - skip audio samples
                tracing::debug!("Audio ahead by {}ms, skipping samples", -drift_ms);
                self.stats.audio_samples_skipped += 1;
                return SyncAction::SkipAudioSamples;
            }
        }

        SyncAction::Continue
    }

    /// Get next video frame (removes from buffer)
    pub fn pop_video_frame(&mut self) -> Option<TimestampedFrame> {
        self.video_buffer.pop_front()
    }

    /// Get next audio samples (removes from buffer)
    pub fn pop_audio_samples(&mut self) -> Option<TimestampedAudio> {
        self.audio_buffer.pop_front()
    }

    /// Drop current video frame
    pub fn drop_video_frame(&mut self) {
        self.video_buffer.pop_front();
    }

    /// Drop current audio samples
    pub fn drop_audio_samples(&mut self) {
        self.audio_buffer.pop_front();
    }

    /// Get synchronization statistics
    pub fn stats(&self) -> SyncStats {
        self.stats
    }

    /// Get video buffer level (0.0 - 1.0)
    pub fn video_buffer_level(&self) -> f32 {
        self.video_buffer.len() as f32 / self.max_video_buffer as f32
    }

    /// Get audio buffer level (0.0 - 1.0)
    pub fn audio_buffer_level(&self) -> f32 {
        self.audio_buffer.len() as f32 / self.max_audio_buffer as f32
    }

    /// Reset sync engine
    pub fn reset(&mut self) {
        self.video_buffer.clear();
        self.audio_buffer.clear();
        self.video_drift_ms = 0;
        self.audio_drift_ms = 0;
        self.stats = SyncStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_engine() {
        let mut engine = SyncEngine::new(50, 16, 64);

        // Add frames with same PTS - should be in sync
        engine.add_video_frame(0, vec![0; 100], 640, 480);
        engine.add_audio_samples(0, vec![0.0; 1000]);

        assert_eq!(engine.sync(), SyncAction::Continue);

        // Add video ahead of audio by 100ms - should drop video
        engine.add_video_frame(100_000, vec![0; 100], 640, 480);
        assert_eq!(engine.sync(), SyncAction::DropVideoFrame);
    }

    #[test]
    fn test_buffer_overflow() {
        let mut engine = SyncEngine::new(50, 2, 4);

        // Fill buffer
        engine.add_video_frame(0, vec![0; 100], 640, 480);
        engine.add_video_frame(1000, vec![0; 100], 640, 480);

        // Should be full
        assert_eq!(engine.video_buffer.len(), 2);

        // Add one more - oldest should be dropped
        engine.add_video_frame(2000, vec![0; 100], 640, 480);
        assert_eq!(engine.video_buffer.len(), 2);
        assert_eq!(engine.stats.video_frames_dropped, 1);
    }
}
