//! Rewind session state: capture schedule, held/released transitions, playback
//! pacing and the disable rules (config off, states refused by `save_state_policy`,
//! serialize failure, achievement hardcore while its gate is active: no capture, no
//! playback, menu reason; it works again once the session drops to softcore).

use std::time::Instant;

use config_layers::RewindConfig;
use libretro_host::{CoreSession, HostError, SavestateContext};
use system_catalog::SaveStateQuality;

use crate::emulation_main_loop::save_state_policy::state_refusal;

use crate::frame_pacer::FramePacer;
use crate::hardcore_gate::{HardcoreGate, REWIND_REFUSED};
use crate::rewind_ring::RewindRing;

/// Real frames per capture (measured: see the phase-04 report). Playback steps one
/// capture per frame, so rewinding runs at this multiple of normal speed.
pub(crate) const REWIND_GRANULARITY: u32 = 2;
pub(super) const CONTEXT: SavestateContext = SavestateContext::RunaheadSameInstance;
const UNAVAILABLE_NOTICE: &str = "Rewind unavailable";

/// Rewind state for one session.
pub(crate) struct Rewind {
    /// `None`: off in the config or unavailable this session (no history memory used).
    ring: Option<RewindRing>,
    /// Reused serialize buffer for frames run-ahead did not serialize.
    buf: Vec<u8>,
    frames_since_capture: u32,
    /// Paces playback at core fps whatever the redraw rate (vsync or uncapped).
    pacer: FramePacer,
    rewinding: bool,
    /// On in the config but refused (`unavailable`): noticed on the first press.
    refused: bool,
    /// Why this core's states cannot rewind (whether or not rewind is on).
    unavailable: Option<&'static str>,
    notice: Option<&'static str>,
    /// Achievement hardcore (inactive without a session).
    hardcore: HardcoreGate,
}

/// What one playback redraw does.
pub(crate) enum StepBack<'a> {
    /// No capture due yet at core fps.
    Wait,
    /// Restore this state.
    Restore(&'a [u8]),
    /// History exhausted: hold the current picture.
    StartOfBuffer,
}

impl Rewind {
    /// From `rewind.enabled` / `rewind.buffer_mb`; refused for states that are
    /// incomplete or not frame-exact (`state_refusal`).
    pub(crate) fn new(
        config: &RewindConfig,
        session: &CoreSession,
        quality: SaveStateQuality,
    ) -> Self {
        let unavailable = state_refusal(session.serialization_quirks().incomplete(), quality);
        let refused = config.enabled && unavailable.is_some();
        if let Some(reason) = unavailable.filter(|_| refused) {
            tracing::warn!(reason, "rewind disabled");
        }
        let budget = config.buffer_mb as usize * 1024 * 1024;
        Self {
            ring: (config.enabled && !refused).then(|| RewindRing::new(budget)),
            buf: Vec::new(),
            frames_since_capture: 0,
            pacer: FramePacer::new(session.av_info().timing.fps),
            rewinding: false,
            refused,
            unavailable,
            notice: None,
            hardcore: HardcoreGate::default(),
        }
    }

    /// Shares the achievement session's hardcore state (rewind inert while active).
    pub(crate) fn set_hardcore_gate(&mut self, gate: HardcoreGate) {
        self.hardcore = gate;
    }

    pub(crate) fn hardcore_active(&self) -> bool {
        self.hardcore.active()
    }

    /// Why rewind cannot be used now (pause menu), set even while off.
    pub(crate) fn unavailable_reason(&self) -> Option<&'static str> {
        (self.hardcore_active().then_some(REWIND_REFUSED)).or(self.unavailable)
    }

    /// After each real-timeline frame. `state`: that frame's state if run-ahead
    /// already serialized it; `progress`: the achievement progress, asked only when a
    /// capture is taken.
    pub(crate) fn after_real_frame(
        &mut self,
        session: &mut CoreSession,
        state: Option<&[u8]>,
        progress: impl FnOnce() -> Option<Vec<u8>>,
    ) {
        let Some(ring) = self.ring.as_mut().filter(|_| !self.hardcore.active()) else {
            return;
        };
        self.frames_since_capture += 1;
        if self.frames_since_capture < REWIND_GRANULARITY {
            return;
        }
        let captured = match state {
            Some(state) => Ok(state),
            None => session
                .serialize_into(&mut self.buf, CONTEXT)
                .map(|_| self.buf.as_slice()),
        };
        match captured {
            Ok(state) => {
                ring.push_with_progress(state, progress().as_deref());
                self.frames_since_capture = 0;
            }
            Err(HostError::SerializeNotReady) => {} // retried next frame
            Err(err) => self.disable(&format!("serialize failed: {err}")),
        }
    }

    /// Feeds the held input; returns `Some(rewinding)` when playback starts or stops.
    pub(crate) fn set_held(&mut self, pressed: bool) -> Option<bool> {
        if pressed && std::mem::take(&mut self.refused) {
            self.notice = Some(UNAVAILABLE_NOTICE);
        }
        let held = pressed && self.ring.is_some() && !self.hardcore_active();
        if held == self.rewinding {
            return None;
        }
        self.rewinding = held;
        if held {
            self.pacer.reset();
        } else if let Some(ring) = &self.ring {
            let (entries, bytes) = (ring.len(), ring.bytes_used());
            tracing::debug!(entries, bytes, "rewind released");
        }
        Some(held)
    }

    pub(crate) fn is_rewinding(&self) -> bool {
        self.rewinding
    }

    /// Steps back one capture per frame due at core fps.
    pub(crate) fn step_back(&mut self, now: Instant) -> StepBack<'_> {
        match self.pacer.frames_due(now) {
            0 => StepBack::Wait,
            due => self
                .pop_captures(due)
                .map_or(StepBack::StartOfBuffer, StepBack::Restore),
        }
    }

    /// Pops up to `count` captures; the state reached, or `None` if none was left.
    pub(crate) fn pop_captures(&mut self, count: u32) -> Option<&[u8]> {
        let ring = self.ring.as_mut()?;
        let mut stepped = false;
        for _ in 0..count {
            if ring.pop().is_none() {
                break;
            }
            stepped = true;
        }
        ring.current().filter(|_| stepped)
    }

    /// Achievement progress of the capture playback restored last.
    pub(crate) fn current_progress(&self) -> Option<&[u8]> {
        self.ring.as_ref()?.current_progress()
    }

    /// Forgets the history (slot load, core reset): the old timeline is gone.
    pub(crate) fn clear(&mut self) {
        if let Some(ring) = self.ring.as_mut() {
            ring.clear();
        }
        self.frames_since_capture = 0;
    }

    /// On-screen message to show once (rewind unavailable).
    pub(crate) fn take_notice(&mut self) -> Option<&'static str> {
        self.notice.take()
    }

    pub(super) fn disable(&mut self, reason: &str) {
        tracing::warn!(reason, "rewind disabled for this session");
        self.ring = None;
        self.notice = Some(UNAVAILABLE_NOTICE);
    }
}
