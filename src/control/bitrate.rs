use crate::network::{ControlMessage, NetworkStats};
use std::time::Instant;

/// Adaptive bitrate controller using AIMD (Additive Increase, Multiplicative Decrease)
pub struct BitrateController {
    current_bitrate: u32,
    min_bitrate: u32,
    max_bitrate: u32,
    last_adjustment: Instant,
    adjustment_interval_ms: u64,

    // AIMD parameters
    increase_step: u32,   // Additive increase (Mbps)
    decrease_factor: f64, // Multiplicative decrease factor

    // Quality thresholds
    rtt_threshold_ms: f64,
    loss_threshold_percent: f64,
}

impl BitrateController {
    pub fn new(initial_bitrate: u32, min_bitrate: u32, max_bitrate: u32) -> Self {
        Self {
            current_bitrate: initial_bitrate,
            min_bitrate,
            max_bitrate,
            last_adjustment: Instant::now(),
            adjustment_interval_ms: 1000, // Adjust every 1 second
            increase_step: 1,             // Increase by 1 Mbps
            decrease_factor: 0.75,        // Decrease by 25%
            rtt_threshold_ms: 200.0,      // Poor network if RTT > 200ms
            loss_threshold_percent: 2.0,  // Poor network if loss > 2%
        }
    }

    /// Update bitrate based on network statistics
    pub fn update(&mut self, stats: &NetworkStats) -> Option<ControlMessage> {
        // Only adjust every adjustment_interval_ms
        if self.last_adjustment.elapsed().as_millis() < self.adjustment_interval_ms as u128 {
            return None;
        }

        let quality = stats.quality_score();
        let should_decrease =
            stats.rtt_ms > self.rtt_threshold_ms || stats.packet_loss > self.loss_threshold_percent;

        let new_bitrate = if should_decrease {
            // Multiplicative decrease
            ((self.current_bitrate as f64 * self.decrease_factor) as u32).max(self.min_bitrate)
        } else if quality > 0.8 {
            // Additive increase (only if quality is good)
            (self.current_bitrate + self.increase_step).min(self.max_bitrate)
        } else {
            // Keep current bitrate
            self.current_bitrate
        };

        if new_bitrate != self.current_bitrate {
            self.current_bitrate = new_bitrate;
            self.last_adjustment = Instant::now();
            Some(ControlMessage::SetBitrate(new_bitrate))
        } else {
            None
        }
    }

    /// Get current bitrate
    pub fn current_bitrate(&self) -> u32 {
        self.current_bitrate
    }

    /// Manually set bitrate
    pub fn set_bitrate(&mut self, bitrate: u32) -> Option<ControlMessage> {
        let clamped = bitrate.clamp(self.min_bitrate, self.max_bitrate);
        if clamped != self.current_bitrate {
            self.current_bitrate = clamped;
            Some(ControlMessage::SetBitrate(clamped))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitrate_adjustment() {
        let mut controller = BitrateController::new(8, 2, 20);

        // Good network - should increase
        let good_stats = NetworkStats {
            rtt_ms: 50.0,
            packet_loss: 0.1,
            bandwidth_mbps: 20.0,
            ..Default::default()
        };

        std::thread::sleep(std::time::Duration::from_millis(1100));
        let msg = controller.update(&good_stats);
        assert!(msg.is_some());
        assert_eq!(controller.current_bitrate(), 9);

        // Poor network - should decrease
        let poor_stats = NetworkStats {
            rtt_ms: 300.0,
            packet_loss: 5.0,
            bandwidth_mbps: 5.0,
            ..Default::default()
        };

        std::thread::sleep(std::time::Duration::from_millis(1100));
        let msg = controller.update(&poor_stats);
        assert!(msg.is_some());
        assert!(controller.current_bitrate() < 9);
    }
}
