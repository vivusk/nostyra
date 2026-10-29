//! Emulation cost per tick (all `retro_run` calls of one redraw, plus run-ahead
//! serialize/unserialize), logged at debug level every 10 s as average and max.
//! Backs the M3 criterion "emulate time < 8 ms at run-ahead 2" (`RUST_LOG=debug`).

use std::time::{Duration, Instant};

/// Time between log lines.
const LOG_EVERY: Duration = Duration::from_secs(10);

pub(crate) struct EmulateTimeStats {
    window_start: Option<Instant>,
    ticks: u32,
    frames: u32,
    total: Duration,
    max: Duration,
}

impl EmulateTimeStats {
    pub(crate) fn new() -> Self {
        Self {
            window_start: None,
            ticks: 0,
            frames: 0,
            total: Duration::ZERO,
            max: Duration::ZERO,
        }
    }

    /// Records one tick that emulated `due` real frames in `elapsed` (ticks with no
    /// frame due are ignored); logs and restarts the window every `LOG_EVERY`.
    pub(crate) fn record(&mut self, due: u32, elapsed: Duration, now: Instant, run_ahead: u8) {
        if due == 0 {
            return;
        }
        let start = *self.window_start.get_or_insert(now);
        self.ticks += 1;
        self.frames += due;
        self.total += elapsed;
        self.max = self.max.max(elapsed);
        if now.duration_since(start) >= LOG_EVERY {
            tracing::debug!(
                avg_ms = format_args!("{:.2}", self.average().as_secs_f64() * 1000.0),
                max_ms = format_args!("{:.2}", self.max.as_secs_f64() * 1000.0),
                ticks = self.ticks,
                frames = self.frames,
                run_ahead,
                "emulate time per tick"
            );
            *self = Self::new();
        }
    }

    fn average(&self) -> Duration {
        self.total / self.ticks.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_ticks_and_resets_after_logging() {
        let t0 = Instant::now();
        let mut stats = EmulateTimeStats::new();
        stats.record(0, Duration::from_millis(50), t0, 0); // nothing due: ignored
        stats.record(1, Duration::from_millis(2), t0, 2);
        stats.record(2, Duration::from_millis(4), t0 + Duration::from_secs(1), 2);
        assert_eq!((stats.ticks, stats.frames), (2, 3));
        assert_eq!(stats.average(), Duration::from_millis(3));
        assert_eq!(stats.max, Duration::from_millis(4));
        stats.record(1, Duration::from_millis(1), t0 + LOG_EVERY, 2);
        assert_eq!(
            (stats.ticks, stats.total),
            (0, Duration::ZERO),
            "window restarted"
        );
    }
}
