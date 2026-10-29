//! Fast-forward while the `fast_forward_hold` input is held (default `` ` `` or R2):
//! emulation runs uncapped with frame skip
//! (the core skips video/audio rendering for all but one frame per preview interval,
//! so time goes to emulation rather than rendering and vsync waits), audio is muted
//! so the queue cannot pile up, and the window title shows the state. Normal pacing
//! resumes on release. Also hosts the per-redraw choice between rewind playback
//! (held rewind wins), fast-forward and normal speed; normal speed goes through
//! `run_ahead` (plain frames when it is off) and feeds rewind captures, the other
//! two bypass both.

use std::cell::Cell;
use std::time::{Duration, Instant};

use libretro_host::{AudioSink, CoreSession};

use super::rewind_playback::Rewind;
use super::run_ahead::RealFrameHooks;
use super::EmulationApp;
use crate::achievement_session::{self, AchievementSession};
use crate::game_save_data::GameSaveData;
use crate::loop_timing::LoopTiming;

/// Time between preview frames while fast-forwarding (~20 fps on screen). Frames in
/// between run with rendering skipped; a longer interval is faster but choppier.
const PREVIEW_INTERVAL: Duration = Duration::from_millis(50);

/// Window title status while fast-forwarding.
pub(super) const FAST_FORWARD_STATUS: &str = "Fast-forward ▶▶";

impl EmulationApp<'_> {
    /// This redraw's emulation: rewind playback while rewind is held, uncapped while
    /// fast-forward is held, else the frames `LoopTiming` says are due (with run-ahead
    /// when enabled).
    pub(super) fn emulate_due_frames(&mut self) -> anyhow::Result<()> {
        if self.rewind_if_held()? {
            return Ok(());
        }
        if self.update_fast_forward() {
            return self.run_fast_forward_slice();
        }
        let due = self.timing.frames_due(Instant::now());
        let started = Instant::now();
        let mut hooks = RealTimeline {
            timing: &mut self.timing,
            saves: &mut self.saves,
            rewind: &mut self.rewind,
            turbo_clock: &self.turbo_clock,
            achievements: &mut self.achievements,
        };
        self.run_ahead.run_tick(
            due,
            &mut self.session,
            &mut self.frame,
            &mut self.input,
            &mut hooks,
        )?;
        let now = Instant::now();
        let run_ahead = self.run_ahead.active_frames();
        self.emulate_stats
            .record(due, now - started, now, run_ahead);
        if let Some(notice) = self.run_ahead.take_notice() {
            self.status.show(notice, now);
        }
        Ok(())
    }

    /// Reads the held state; on a change updates the title and, on release, resets the
    /// pacer so normal speed does not try to catch up. Returns whether it is held.
    fn update_fast_forward(&mut self) -> bool {
        let held = self.input.fast_forward_held();
        if held != self.fast_forwarding {
            self.fast_forwarding = held;
            if !held {
                self.timing.resume_normal_speed();
            }
            if let Some(window) = &self.window {
                let status = if held { FAST_FORWARD_STATUS } else { "" };
                window.set_title(&self.saves.window_title(status));
            }
            tracing::debug!(held, "fast-forward");
        }
        held
    }

    /// Emulates skipped frames back to back for one preview interval, then one rendered
    /// frame for the preview presented after this tick. Audio muted throughout.
    fn run_fast_forward_slice(&mut self) -> anyhow::Result<()> {
        let deadline = Instant::now() + PREVIEW_INTERVAL;
        self.session.set_frame_skip(true);
        let skipped = self.run_skipped_frames_until(deadline);
        self.session.set_frame_skip(false);
        skipped?;
        self.session
            .run_frame(&mut self.frame, self.timing.muted_sink(), &mut self.input)?;
        self.saves.tick(&mut self.session);
        achievement_session::after_real_frame(&mut self.achievements, &self.session);
        next_real_frame(&self.turbo_clock);
        Ok(())
    }

    fn run_skipped_frames_until(&mut self, deadline: Instant) -> anyhow::Result<()> {
        while Instant::now() < deadline {
            self.session
                .run_frame(&mut self.frame, self.timing.muted_sink(), &mut self.input)?;
            self.saves.tick(&mut self.session);
            achievement_session::after_real_frame(&mut self.achievements, &self.session);
            next_real_frame(&self.turbo_clock);
        }
        Ok(())
    }
}

/// Advances the turbo A phase by one real-timeline frame.
fn next_real_frame(turbo_clock: &Cell<u64>) {
    turbo_clock.set(turbo_clock.get().wrapping_add(1));
}

/// Real-timeline frame hooks for the normal-speed loop: audio pacing (DRC), the
/// periodic SRAM flush, rewind captures (with achievement progress), the turbo A
/// phase and achievements' `do_frame`, which must never see run-ahead's hidden frames.
struct RealTimeline<'a> {
    timing: &'a mut LoopTiming,
    saves: &'a mut GameSaveData,
    rewind: &'a mut Rewind,
    turbo_clock: &'a Cell<u64>,
    achievements: &'a mut Option<AchievementSession>,
}

impl RealFrameHooks for RealTimeline<'_> {
    fn before_real_frame(&mut self) {
        self.timing.before_frame();
    }

    fn audio_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        self.timing.audio_sink()
    }

    fn muted_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        self.timing.muted_sink()
    }

    fn after_real_frame(&mut self, session: &mut CoreSession, state: Option<&[u8]>) {
        self.timing.after_frame();
        self.saves.tick(session);
        achievement_session::after_real_frame(self.achievements, session);
        let achievements = &*self.achievements;
        let progress = || {
            achievements
                .as_ref()
                .and_then(AchievementSession::rewind_progress)
        };
        self.rewind.after_real_frame(session, state, progress);
        next_real_frame(self.turbo_clock);
    }
}
