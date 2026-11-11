//! Last step of a redraw: pick up shader build results, hand the presenter the
//! overlay (the pause menu while open, else the netplay waiting room while a
//! session is not running, else the on-screen status composed with the achievement
//! layers when a session runs, else the netplay HUD with the status; blended after
//! the shader at output resolution) and present.

use std::time::{Duration, Instant};

use video_renderer::PresentOutcome;

use super::EmulationApp;
use crate::netplay_overlay::NetplayOverlay;

impl EmulationApp<'_> {
    /// Presents the current frame (no-op until the window exists).
    pub(super) fn present(&mut self) -> anyhow::Result<()> {
        self.report_shader_load();
        if self.netplay.as_ref().is_some_and(|np| np.window_hidden) {
            return Ok(()); // nothing to show; an invisible swapchain may block
        }
        let Some(presenter) = self.presenter.as_mut() else {
            return Ok(());
        };
        let output = presenter.output_size();
        let now = Instant::now();
        let waiting_room = self
            .netplay
            .as_ref()
            .is_some_and(|np| NetplayOverlay::shows_waiting_room(&np.runtime));
        let overlay = if self.menu.is_open() {
            let badges = self.achievements.as_mut().map(|a| a.badges_mut());
            self.menu.overlay(output, self.saves.rom_name(), badges)
        } else if let (true, Some(np)) = (waiting_room, self.netplay.as_mut()) {
            Some(np.overlay.waiting_room(output, &np.runtime))
        } else {
            let game = presenter.game_viewport();
            let layers = match (game, self.achievements.as_mut()) {
                (Some(game), Some(achievements)) => {
                    let (status, fast) = (&mut self.status, self.fast_forwarding);
                    achievements.overlay((output, game), now, status, fast)
                }
                (Some(game), None) if self.netplay.is_none() => {
                    self.status.overlay(output, game, now)
                }
                _ => None,
            };
            match (layers, game, self.netplay.as_mut()) {
                (Some(layers), _, _) => Some(layers),
                (None, Some(game), Some(np)) => {
                    let status = self.status.current(now);
                    let hud = (np.hud_on, status);
                    np.overlay.hud((output, game), &np.runtime, hud, now)
                }
                (None, _, _) => None,
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
