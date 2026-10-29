//! Window creation and display-refresh-driven sync / present-mode selection.

use std::time::Instant;

use video_renderer::PresentMode;
use winit::event_loop::ActiveEventLoop;

use super::pause_glue::MENU_HINT;
use super::EmulationApp;
use crate::runner_effective_config::scaling_mode;
use crate::runner_window::{display_hz, open_game_window};

impl EmulationApp<'_> {
    pub(super) fn create_window(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        let av = self.session.av_info();
        let gl_in_use = self.session.uses_hw_context();
        let (window, presenter) =
            open_game_window(event_loop, &av, self.start_fullscreen, gl_in_use)?;
        window.set_title(&self.saves.window_title(""));
        self.window = Some(window);
        self.presenter = Some(presenter);
        self.apply_scaling();
        self.request_configured_shader();
        self.status.show(MENU_HINT, Instant::now());
        self.refresh_sync_mode(true);
        Ok(())
    }

    /// `video.scaling` → presenter (window creation, pause-menu change).
    pub(super) fn apply_scaling(&mut self) {
        if let Some(presenter) = self.presenter.as_mut() {
            presenter.set_scaling_mode(scaling_mode(self.config.video.scaling));
        }
    }

    /// Re-reads the monitor refresh (start, window moved / DPI change) and
    /// reselects sync + present mode when it changed (winit has no monitor event).
    pub(super) fn refresh_sync_mode(&mut self, force: bool) {
        let Some(window) = &self.window else { return };
        let hz = display_hz(window);
        if !force && hz == self.display_hz {
            return;
        }
        self.display_hz = hz;
        let prefs = self.timing.select_mode(hz);
        self.apply_present_mode(prefs);
    }

    pub(super) fn apply_present_mode(&mut self, prefs: &'static [PresentMode]) {
        if let Some(presenter) = self.presenter.as_mut() {
            let mode = presenter.set_present_mode(prefs);
            self.timing.warn_if_fifo_throttles(mode, self.display_hz);
        }
    }
}
