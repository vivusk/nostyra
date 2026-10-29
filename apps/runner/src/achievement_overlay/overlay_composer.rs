//! One overlay canvas per redraw while the menu is closed and a session runs: the
//! on-screen status line (5×7 font, bottom-left, unchanged look) plus the achievement
//! layers (popup at the top, indicators at the bottom-right). The canvas is repainted
//! only when the window, the picture, the status text or an achievement layer
//! changed; with nothing to show there is no overlay at all (no upload, no blend).

use video_renderer::{OverlayCanvas, TtfText};

use super::indicator_view::draw_indicators;
use super::popup_view::draw_popup;
use super::{ui_px, AchievementOverlay};
use crate::on_screen_status::{draw_status, text_scale};

/// What the canvas was last painted for.
#[derive(PartialEq)]
struct ComposeKey {
    output: (u32, u32),
    game: [u32; 4],
    status: Option<String>,
    revisions: (u64, u64, u64),
}

#[derive(Default)]
pub struct OverlayComposer {
    canvas: OverlayCanvas,
    /// Achievement text (fonts parsed on the first popup / indicator only).
    text: TtfText,
    drawn: Option<ComposeKey>,
}

impl OverlayComposer {
    /// The composed overlay for an `output`-sized window with the picture at `game`
    /// (x, y, w, h); `None` when neither the status nor an achievement layer shows.
    pub fn compose(
        &mut self,
        output: (u32, u32),
        game: [u32; 4],
        status: Option<&str>,
        overlay: &mut AchievementOverlay,
    ) -> Option<&OverlayCanvas> {
        let achievements = overlay.popups.current().is_some() || !overlay.indicators.is_empty();
        if status.is_none() && !achievements {
            self.drawn = None;
            return None;
        }
        let revisions = overlay.revisions();
        let unchanged = self.drawn.as_ref().is_some_and(|key| {
            key.output == output
                && key.game == game
                && key.status.as_deref() == status
                && key.revisions == revisions
        });
        if !unchanged {
            self.paint(output, game, status, overlay);
            self.drawn = Some(ComposeKey {
                output,
                game,
                status: status.map(str::to_owned),
                revisions,
            });
        }
        Some(&self.canvas)
    }

    fn paint(
        &mut self,
        output: (u32, u32),
        game: [u32; 4],
        status: Option<&str>,
        overlay: &mut AchievementOverlay,
    ) {
        self.canvas.reset(output.0, output.1);
        if let Some(status) = status {
            draw_status(&mut self.canvas, game, status, text_scale(output.1));
        }
        let px = ui_px(output.1);
        let AchievementOverlay {
            popups,
            indicators,
            badges,
            ..
        } = overlay;
        if let Some(popup) = popups.current() {
            let tools = (&mut self.text, &mut *badges);
            draw_popup(&mut self.canvas, tools, output, popup, px);
        }
        let tools = (&mut self.text, &mut *badges);
        draw_indicators(&mut self.canvas, tools, game, indicators, px);
    }
}
