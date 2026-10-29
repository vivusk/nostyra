//! Window loop ↔ achievement session: one call per loop event (pause idle, reset,
//! disc swap, state progress, menu list page, shutdown, hardcore gate and pause
//! limit), so the loop files only gain single lines. Every call is a no-op without
//! a session (logged out, off, unsupported system).

use super::EmulationApp;
use crate::achievement_session::{progress_of, AchievementSession};
use crate::hardcore_gate::HardcoreGate;

impl EmulationApp<'_> {
    /// Hands the session started after the core loaded to the loop and shares its
    /// hardcore state with the state slots and rewind (before any startup load).
    pub fn attach_achievements(&mut self, mut achievements: Option<AchievementSession>) {
        if let Some(session) = achievements.as_mut() {
            session.attach_window(self.sdl.audio()); // badges + chime: drawn here
        }
        self.achievements = achievements;
        let gate = self.hardcore_gate();
        self.saves.set_hardcore_gate(gate.clone());
        self.rewind.set_hardcore_gate(gate);
    }

    /// The session's hardcore state (inactive without a session).
    pub(super) fn hardcore_gate(&self) -> HardcoreGate {
        self.achievements
            .as_ref()
            .map(AchievementSession::hardcore_gate)
            .unwrap_or_default()
    }

    /// "Pause available in N s" when hardcore refuses a pause right now.
    pub(super) fn hardcore_pause_refusal(&self) -> Option<String> {
        let fps = self.session.av_info().timing.fps;
        self.achievements.as_ref()?.pause_refusal(fps)
    }

    /// Hex achievement progress for a state being saved now.
    pub(super) fn achievement_progress(&self) -> Option<String> {
        progress_of(&self.achievements)
    }

    /// A state was loaded with `progress` (absent → reset; set still loading → pending).
    pub(super) fn apply_state_progress(&mut self, progress: Option<String>) {
        if let Some(achievements) = &mut self.achievements {
            achievements.apply_loaded_progress(progress.as_deref());
        }
    }

    /// Paused redraw (menu open): keep pings / retries / results flowing; an open
    /// achievement page follows a set load that completes meanwhile.
    pub(super) fn achievements_idle(&mut self) {
        if let Some(achievements) = &mut self.achievements {
            achievements.idle();
        }
        self.refresh_achievement_list(false);
    }

    /// Hands the menu the achievement page: when it `opened`, or after the list
    /// changed (no session: no page, no root item).
    pub(super) fn refresh_achievement_list(&mut self, opened: bool) {
        let Some(achievements) = self.achievements.as_mut() else {
            return;
        };
        if achievements.take_list_changed() || opened {
            self.menu
                .set_achievements(achievements.achievement_list_page());
        }
    }

    pub(super) fn achievements_reset(&mut self) {
        if let Some(achievements) = &mut self.achievements {
            achievements.reset();
        }
    }

    pub(super) fn achievements_disc_swapped(&mut self, index: u32) {
        if let Some(achievements) = &mut self.achievements {
            achievements.disc_swapped(index);
        }
    }

    /// End of the session, after the resume point: before the core unloads (D4).
    pub(super) fn shutdown_achievements(&mut self) {
        if let Some(achievements) = self.achievements.take() {
            achievements.shutdown();
        }
    }
}
