//! The session side of hardcore (M15 phase 04): it owns the [`HardcoreGate`] the
//! state slots and rewind read, drops to softcore for the rest of the session (our
//! own reasons: disallowed core option, a disc we cannot identify), follows rc_client
//! when it disables hardcore itself (unknown disc), and asks rc_client whether a
//! pause is allowed now. Hardcore is decided at launch only: nothing turns it on.

use super::AchievementSession;
use crate::hardcore_gate::HardcoreGate;

impl AchievementSession {
    /// The shared hardcore state (a clone: lifts are seen by every holder).
    pub fn hardcore_gate(&self) -> HardcoreGate {
        self.hardcore.clone()
    }

    /// Hardcore off for the rest of the session: rc_client to softcore, every gate
    /// lifted, the player told why. No-op when already softcore.
    pub fn drop_to_softcore(&mut self, reason: &str) {
        if !self.hardcore.lift() {
            return;
        }
        if let Some(client) = &self.client {
            client.set_hardcore_enabled(false);
        }
        tracing::warn!(reason, "hardcore disabled for this session");
        self.router.notify(format!("Hardcore disabled: {reason}"));
    }

    /// rc_client disabled hardcore on its own (unknown disc inserted; the router
    /// already told the player): the gates follow.
    pub(super) fn follow_client_hardcore(&mut self) {
        let client_softcore = self.client.as_ref().is_some_and(|c| !c.hardcore_enabled());
        if client_softcore && self.hardcore.lift() {
            tracing::warn!("rc_client disabled hardcore: gates lifted");
        }
    }

    /// `Some("Pause available in N s")` when hardcore refuses a pause right now
    /// (`rc_client_can_pause`; `fps` converts its frame count). Asking counts as a
    /// pause for rc_client, so call it only when the player asks to pause.
    pub fn pause_refusal(&self, fps: f64) -> Option<String> {
        if !self.hardcore.active() {
            return None;
        }
        let (allowed, frames) = self.client.as_ref()?.can_pause();
        (!allowed).then(|| pause_wait_text(frames, fps))
    }
}

/// "Pause available in N s", N = `frames` at `fps` rounded up (at least 1).
pub(super) fn pause_wait_text(frames: u32, fps: f64) -> String {
    let fps = if fps.is_finite() && fps > 1.0 {
        fps
    } else {
        60.0
    };
    let seconds = (f64::from(frames) / fps).ceil().max(1.0);
    format!("Pause available in {seconds:.0} s")
}
