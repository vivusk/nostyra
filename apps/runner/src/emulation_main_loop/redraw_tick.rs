//! One redraw of the game window: SDL pump → pause menu (non-pausing in netplay)
//! → pointer → startup load → due frames → periodic resume point → core notices →
//! AV changes → present; plus the launcher's `Started` once the window is up.

use winit::event_loop::ActiveEventLoop;

use super::EmulationApp;

impl EmulationApp<'_> {
    pub(super) fn report_started(&self) {
        self.reporter
            .started(&self.session.core_info().name, &self.pending_sessions_dir);
    }

    /// One redraw: input → emulate due frames (none while paused) → present.
    pub(super) fn tick(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        if self.sdl.pump(&mut self.input) {
            event_loop.exit();
            return Ok(());
        }
        if self.update_pause_menu(event_loop) {
            return self.present(); // last frame under the menu
        }
        self.update_pointer();
        self.apply_startup_load();
        self.emulate_due_frames()?;
        self.netplay_after_frames(event_loop);
        self.save_periodic_resume_point();
        self.show_core_messages();
        if let Some(av) = self.session.take_av_change() {
            let prefs = self
                .timing
                .on_av_change(av, self.sdl.audio(), self.display_hz);
            self.apply_present_mode(prefs);
            self.apply_geometry(av.geometry); // picture aspect; window on a layout change
        }
        self.present()
    }
}
