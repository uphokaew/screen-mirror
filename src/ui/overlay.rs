use crate::network::NetworkStats;
use crate::sync::SyncStats;

/// Statistics overlay using egui
pub struct StatsOverlay {
    visible: bool,
    fps: f32,
    latency_ms: f32,
    frame_count: u64,
    last_stats_update: std::time::Instant,
}

impl StatsOverlay {
    pub fn new() -> Self {
        Self {
            visible: true,
            fps: 0.0,
            latency_ms: 0.0,
            frame_count: 0,
            last_stats_update: std::time::Instant::now(),
        }
    }

    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn update_frame(&mut self) {
        self.frame_count += 1;

        // Calculate FPS every second
        let elapsed = self.last_stats_update.elapsed();
        if elapsed.as_secs() >= 1 {
            self.fps = self.frame_count as f32 / elapsed.as_secs_f32();
            self.frame_count = 0;
            self.last_stats_update = std::time::Instant::now();
        }
    }

    pub fn set_latency(&mut self, latency_ms: f32) {
        self.latency_ms = latency_ms;
    }

    /// Render the overlay (placeholder for egui implementation)
    ///
    /// In a full implementation, this would use egui::Context to render
    /// a window with all statistics. For now, we just track the data.
    pub fn render(
        &mut self,
        _ctx: &egui::Context,
        _network_stats: &NetworkStats,
        _sync_stats: &SyncStats,
    ) {
        if !self.visible {
        }

        // Full implementation would render:
        // - FPS counter
        // - Latency (end-to-end)
        // - Network stats (RTT, loss, bitrate)
        // - Sync stats (drift, dropped frames)
        // - Buffer levels
        // - GPU/CPU usage

        // Example egui code (commented out as ctx would need proper setup):
        /*
        egui::Window::new("Performance Stats")
            .default_pos([10.0, 10.0])
            .show(ctx, |ui| {
                ui.heading("Video");
                ui.label(format!("FPS: {:.1}", self.fps));
                ui.label(format!("Latency: {:.1}ms", self.latency_ms));

                ui.separator();
                ui.heading("Network");
                ui.label(format!("RTT: {:.1}ms", network_stats.rtt_ms));
                ui.label(format!("Packet Loss: {:.2}%", network_stats.packet_loss));
                ui.label(format!("Bitrate: {:.1} Mbps", network_stats.bandwidth_mbps));

                ui.separator();
                ui.heading("Synchronization");
                ui.label(format!("Drift: {}ms", sync_stats.current_drift_ms));
                ui.label(format!("Avg Drift: {:.1}ms", sync_stats.avg_drift_ms));
                ui.label(format!("Frames Dropped: {}", sync_stats.video_frames_dropped));
                ui.label(format!("Audio Skipped: {}", sync_stats.audio_samples_skipped));
            });
        */
    }

    /// Get current FPS
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// Get current latency
    pub fn latency_ms(&self) -> f32 {
        self.latency_ms
    }

    /// Get stats summary as string (for logging)
    pub fn stats_summary(&self, network_stats: &NetworkStats, sync_stats: &SyncStats) -> String {
        format!(
            "FPS: {:.1} | Latency: {:.1}ms | RTT: {:.1}ms | Loss: {:.2}% | Drift: {}ms | Dropped: {}",
            self.fps,
            self.latency_ms,
            network_stats.rtt_ms,
            network_stats.packet_loss,
            sync_stats.current_drift_ms,
            sync_stats.video_frames_dropped
        )
    }
}

impl Default for StatsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_overlay() {
        let mut overlay = StatsOverlay::new();
        assert!(overlay.is_visible());

        overlay.toggle_visibility();
        assert!(!overlay.is_visible());

        overlay.set_latency(45.0);
        assert_eq!(overlay.latency_ms(), 45.0);
    }
}
