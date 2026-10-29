//! Achievements during rewind playback: no `do_frame` (playback is not the real
//! timeline), `idle` each rewinding redraw, and every restored capture brings back
//! the achievement progress captured with it (absent → reset).

use super::super::EmulationApp;

impl EmulationApp<'_> {
    /// Each redraw while rewinding.
    pub(in crate::emulation_main_loop) fn rewind_idle_achievements(&mut self) {
        if let Some(achievements) = &mut self.achievements {
            achievements.idle();
        }
    }

    /// After a capture was restored into the core.
    pub(in crate::emulation_main_loop) fn restore_rewind_achievements(&mut self) {
        if let Some(achievements) = &self.achievements {
            achievements.restore_rewind_progress(self.rewind.current_progress());
        }
    }
}
