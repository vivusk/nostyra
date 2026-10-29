//! Last step of a redraw: pick up shader build results, hand the presenter the
//! overlay (the pause menu while open, else the on-screen status composed with the
//! achievement layers when a session runs; blended after the shader at output
//! resolution) and present.

use std::time::{Duration, Instant};

use video_renderer::PresentOutcome;

use super::EmulationApp;

impl EmulationApp<'_> {
    /// Presents the current frame (no-op until the window exists).
    pub(super) fn present(&mut self) -> anyhow::Result<()> {
        self.report_shader_load();
        let Some(presenter) = self.presenter.as_mut() else {
            return Ok(());
        };
        let output = presenter.output_size();
        let now = Instant::now();
        let overlay = if self.menu.is_open() {
            let badges = self.achievements.as_mut().map(|a| a.badges_mut());
            self.menu.overlay(output, self.saves.rom_name(), badges)
        } else {
            match (presenter.game_viewport(), self.achievements.as_mut()) {
                (Some(game), Some(achievements)) => {
                    let (status, fast) = (&mut self.status, self.fast_forwarding);
                    achievements.overlay((output, game), now, status, fast)
                }
                (Some(game), None) => self.status.overlay(output, game, now),
                (None, _) => None,
            }
        };
        presenter.set_overlay(overlay);
        if presenter.render(&mut self.frame)? == PresentOutcome::Skipped {
            // Minimized/occluded: no vsync to block on, so avoid a busy loop.
            std::thread::sleep(Duration::from_millis(4));
        }
        Ok(())
    }
}
