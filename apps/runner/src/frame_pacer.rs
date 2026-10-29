//! Wall-clock frame pacing for the bare loop: decides how many emulated frames
//! are due per redraw so games run at core fps on any display refresh rate.
//!
//! Used only when no audio device is available; otherwise `LoopTiming` paces
//! by vsync or the audio queue.

use std::time::{Duration, Instant};

/// Upper bound per tick so a stall (window drag, debugger) doesn't fast-forward.
const MAX_FRAMES_PER_TICK: u32 = 3;

pub struct FramePacer {
    frame_time: Duration,
    accumulated: Duration,
    last_tick: Option<Instant>,
}

impl FramePacer {
    pub fn new(fps: f64) -> Self {
        Self {
            frame_time: frame_time(fps),
            accumulated: Duration::ZERO,
            last_tick: None,
        }
    }

    /// Updates the target rate (core changed AV info).
    pub fn set_fps(&mut self, fps: f64) {
        self.frame_time = frame_time(fps);
    }

    /// Starts over as if freshly created (after fast-forward: no catch-up frames).
    pub fn reset(&mut self) {
        self.accumulated = Duration::ZERO;
        self.last_tick = None;
    }

    /// Number of frames to emulate now. The first tick always runs one frame.
    pub fn frames_due(&mut self, now: Instant) -> u32 {
        let Some(last) = self.last_tick.replace(now) else {
            return 1;
        };
        self.accumulated += now.saturating_duration_since(last);
        let cap = self.frame_time * MAX_FRAMES_PER_TICK;
        if self.accumulated > cap {
            self.accumulated = cap; // drop backlog instead of fast-forwarding
        }
        let mut due = 0;
        while self.accumulated >= self.frame_time {
            self.accumulated -= self.frame_time;
            due += 1;
        }
        due
    }
}

/// Measures the achieved emulation rate; logs it at `debug` every few seconds
/// (diagnoses pacing without a screen capture: `RUST_LOG=nostyra_runner=debug`).
pub struct FpsMeter {
    window_start: Option<Instant>,
    frames: u32,
}

impl FpsMeter {
    const REPORT_EVERY: Duration = Duration::from_secs(5);

    pub fn new() -> Self {
        Self {
            window_start: None,
            frames: 0,
        }
    }

    /// Records `frames` emulated at `now`; returns the measured fps when a window completes.
    pub fn record(&mut self, now: Instant, frames: u32) -> Option<f64> {
        let start = *self.window_start.get_or_insert(now);
        self.frames += frames;
        let elapsed = now.saturating_duration_since(start);
        if elapsed < Self::REPORT_EVERY {
            return None;
        }
        let fps = f64::from(self.frames) / elapsed.as_secs_f64();
        tracing::debug!(fps = format_args!("{fps:.2}"), "emulation rate");
        self.window_start = Some(now);
        self.frames = 0;
        Some(fps)
    }
}

fn frame_time(fps: f64) -> Duration {
    // Guard against bogus AV info (0 / NaN) by assuming 60 fps.
    let fps = if fps.is_finite() && fps > 1.0 {
        fps
    } else {
        60.0
    };
    Duration::from_secs_f64(1.0 / fps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_core_rate_on_fast_display() {
        // 144 Hz redraws for one second at 60 fps → ~60 frames.
        let mut pacer = FramePacer::new(60.0);
        let start = Instant::now();
        let total: u32 = (0..=144)
            .map(|i| pacer.frames_due(start + Duration::from_secs_f64(i as f64 / 144.0)))
            .sum();
        assert!((60..=61).contains(&total), "{total}");
    }

    #[test]
    fn stall_is_capped() {
        let mut pacer = FramePacer::new(60.0);
        let start = Instant::now();
        pacer.frames_due(start);
        assert_eq!(
            pacer.frames_due(start + Duration::from_secs(5)),
            MAX_FRAMES_PER_TICK
        );
    }

    #[test]
    fn reset_drops_the_backlog() {
        let mut pacer = FramePacer::new(60.0);
        let start = Instant::now();
        pacer.frames_due(start);
        pacer.reset();
        // After a long fast-forward the next tick runs one frame, not a catch-up burst.
        assert_eq!(pacer.frames_due(start + Duration::from_secs(5)), 1);
    }

    #[test]
    fn fps_meter_reports_after_window() {
        let mut meter = FpsMeter::new();
        let start = Instant::now();
        assert_eq!(meter.record(start, 0), None);
        let fps = meter.record(start + FpsMeter::REPORT_EVERY, 300).unwrap();
        assert!((fps - 60.0).abs() < 0.01, "{fps}");
    }

    #[test]
    fn bogus_fps_falls_back_to_60() {
        assert_eq!(frame_time(0.0), Duration::from_secs_f64(1.0 / 60.0));
    }
}
