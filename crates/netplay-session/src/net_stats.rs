//! Snapshot of the session's network health for the HUD and the runner's IPC
//! `PeerStats` event, plus the one-second rollback meter behind it.

use std::time::{Duration, Instant};

/// One remote peer (player or, on the host, spectator) as ggrs sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PeerStats {
    pub handle: usize,
    /// ggrs round trip; `None` until ggrs has one second of data.
    pub ping_ms: Option<u32>,
    /// TURN relayed (filled by the runner from the socket; ggrs cannot tell).
    pub relayed: Option<bool>,
    /// Frames this peer is behind us (positive = we are ahead).
    pub remote_frames_behind: i32,
}

/// HUD / IPC snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetplayStats {
    pub peers: Vec<PeerStats>,
    /// Re-simulated frames during the last full second.
    pub rollback_frames_last_second: u32,
    /// Ticks refused with `PredictionThreshold` since the session started.
    pub stall_count: u32,
    /// A `DesyncDetected` event arrived (cleared by a resync).
    pub desync: bool,
    /// Pacer slowdown the driver asks for (0 or 10).
    pub slowdown_percent: u8,
}

/// Counts re-simulated frames per wall-clock second.
#[derive(Debug, Clone, Copy)]
pub struct RollbackMeter {
    window_start: Instant,
    current: u32,
    last: u32,
}

impl RollbackMeter {
    pub fn new(now: Instant) -> Self {
        Self {
            window_start: now,
            current: 0,
            last: 0,
        }
    }

    /// Adds `frames` re-simulated at `now`.
    pub fn record(&mut self, frames: u32, now: Instant) {
        self.roll(now);
        self.current = self.current.saturating_add(frames);
    }

    /// Re-simulated frames of the last complete second.
    pub fn last_second(&mut self, now: Instant) -> u32 {
        self.roll(now);
        self.last
    }

    fn roll(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.window_start);
        if elapsed >= Duration::from_secs(2) {
            (self.last, self.current) = (0, 0); // a whole quiet second passed
            self.window_start = now;
        } else if elapsed >= Duration::from_secs(1) {
            (self.last, self.current) = (self.current, 0);
            self.window_start += Duration::from_secs(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_reports_the_previous_second() {
        let t0 = Instant::now();
        let mut meter = RollbackMeter::new(t0);
        meter.record(3, t0);
        meter.record(4, t0 + Duration::from_millis(900));
        assert_eq!(meter.last_second(t0 + Duration::from_millis(950)), 0);
        assert_eq!(meter.last_second(t0 + Duration::from_millis(1100)), 7);
        meter.record(1, t0 + Duration::from_millis(1200));
        assert_eq!(meter.last_second(t0 + Duration::from_millis(2100)), 1);
        assert_eq!(meter.last_second(t0 + Duration::from_secs(9)), 0);
    }
}
