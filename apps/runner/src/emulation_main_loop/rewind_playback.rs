//! Hold-to-rewind (`rewind_hold` input, default Backspace / L2, when `rewind.enabled`).
//!
//! Capture: every `REWIND_GRANULARITY` real-timeline frames the state goes into a
//! `RewindRing` (reusing run-ahead's serialize of the same frame when there is
//! one). Only real frames count, so run-ahead's hidden frames, fast-forward and
//! frames not emulated at all (a paused game) never push.
//!
//! Playback: while held, normal emulation, run-ahead and fast-forward are skipped;
//! per capture due at core fps the ring steps back, the state is restored and one
//! frame runs with video only (audio muted) to refresh the picture. An empty ring
//! holds the last picture. Release resumes normal pacing like fast-forward does.
//! Achievements idle meanwhile and follow each restored capture (`rewind_progress`).
//! In achievement hardcore nothing is captured or played back; holding the input
//! only shows why.

use std::time::Instant;

use libretro_host::AvEnable;

use super::run_ahead::VIDEO_ONLY;
use super::EmulationApp;
use crate::hardcore_gate::REWIND_REFUSED_STATUS;

mod rewind_progress;
pub(crate) mod rewind_state;

pub(crate) use rewind_state::Rewind;
use rewind_state::{StepBack, CONTEXT};

const REWINDING_STATUS: &str = "Rewinding";
const START_OF_BUFFER_STATUS: &str = "Rewind: start of buffer";

impl EmulationApp<'_> {
    /// Rewind playback while held. Returns whether this redraw rewound (normal
    /// emulation, run-ahead and fast-forward skipped).
    pub(super) fn rewind_if_held(&mut self) -> anyhow::Result<bool> {
        let now = Instant::now();
        let pressed = self.input.rewind_held();
        if pressed && self.rewind.hardcore_active() {
            self.status.show_while_held(REWIND_REFUSED_STATUS, now);
        }
        let change = self.rewind.set_held(pressed);
        if let Some(notice) = self.rewind.take_notice() {
            self.status.show(notice, now);
        }
        if change == Some(false) {
            // The audio queue drained while muted: refill it, forget the backlog.
            self.timing.resume_normal_speed();
        }
        if !self.rewind.is_rewinding() {
            return Ok(false);
        }
        self.rewind_idle_achievements();
        let state = match self.rewind.step_back(now) {
            StepBack::Wait => return Ok(true),
            StepBack::StartOfBuffer => {
                self.status.show_while_held(START_OF_BUFFER_STATUS, now);
                return Ok(true);
            }
            StepBack::Restore(state) => state,
        };
        if let Err(err) = self.session.unserialize_from(state, CONTEXT) {
            self.rewind.disable(&format!("unserialize failed: {err}"));
            return Ok(true);
        }
        self.restore_rewind_achievements();
        self.session.set_av_enable(VIDEO_ONLY);
        let ran =
            self.session
                .run_frame(&mut self.frame, self.timing.muted_sink(), &mut self.input);
        self.session.set_av_enable(AvEnable::ALL);
        ran?;
        self.status.show_while_held(REWINDING_STATUS, now);
        Ok(true)
    }
}
