//! Wall-clock frame pacing for the bare loop: decides how many emulated frames
//! are due per redraw so games run at core fps on any display refresh rate.
//!
//! Used when no audio device is available (otherwise `LoopTiming` paces by vsync
//! or the audio queue) and, in every audio mode, by netplay (`with_cap` + a speed
//! scale for the ggrs slowdown: the audio-driven modes have nothing to scale).

use std::time::{Duration, Instant};

/// Upper bound per tick so a stall (window drag, debugger) doesn't fast-forward.
pub(crate) const MAX_FRAMES_PER_TICK: u32 = 3;

pub struct FramePacer {
    /// Core frame time × the speed scale.
    frame_time: Duration,
    base_frame_time: Duration,
    scale: f64,
    cap: u32,
    accumulated: Duration,
    last_tick: Option<Instant>,
}

impl FramePacer {
    pub fn new(fps: f64) -> Self {
        Self::with_cap(fps, MAX_FRAMES_PER_TICK)
    }

    /// At most `cap` frames per tick (netplay: 12, so a spectator catches up).
    pub fn with_cap(fps: f64, cap: u32) -> Self {
        let base = frame_time(fps);
        Self {
            frame_time: base,
            base_frame_time: base,
            scale: 1.0,
            cap: cap.max(1),
            accumulated: Duration::ZERO,
            last_tick: None,
        }
    }

    /// Updates the target rate (core changed AV info).
    pub fn set_fps(&mut self, fps: f64) {
        self.base_frame_time = frame_time(fps);
        self.frame_time = self.base_frame_time.mul_f64(self.scale);
    }

    /// Effective frame time = core frame time × `scale` (netplay slowdown: 1.10
    /// while ahead of the slowest peer, 1.0 otherwise).
    pub fn set_speed_scale(&mut self, scale: f64) {
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 2.0)
        } else {
            1.0
        };
        if scale != self.scale {
            self.scale = scale;
            self.frame_time = self.base_frame_time.mul_f64(scale);
        }
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
        let cap = self.frame_time * self.cap;
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
#[path = "frame_pacer_tests.rs"]
mod tests;
