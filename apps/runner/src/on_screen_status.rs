//! On-screen status: a short message such as "Slot 3 · Saved" in the 5×7 pixel font
//! on a translucent dark box at the bottom-left of the game picture for a few
//! seconds. Drawn into an overlay canvas at output resolution that the presenter
//! blends over the picture after the shader stage, so neither the core frame (save
//! states, thumbnails) nor CRT shaders ever touch it.

use std::time::{Duration, Instant};

use video_renderer::{text_size, OverlayCanvas, ViewportRect};

/// How long a message stays on screen.
pub const STATUS_DURATION: Duration = Duration::from_secs(3);
/// How long a held-state message stays after its key is released.
const HELD_LINGER: Duration = Duration::from_millis(250);
/// Box padding and distance from the picture edges, in font pixels.
const PADDING: u32 = 3;
const MARGIN: u32 = 4;
/// 65 % black. Blended in linear light on the sRGB surface, so it reads slightly lighter than gamma-space dimming would.
const BOX_RGBA: [u8; 4] = [0, 0, 0, 166];
const TEXT_RGBA: [u8; 4] = [0xF4, 0xF4, 0xF6, 0xFF];

/// Output size + game rect (whole pixels) the canvas was last drawn for.
type Layout = ((u32, u32), [u32; 4]);

#[derive(Default)]
pub struct OnScreenStatus {
    text: String,
    until: Option<Instant>,
    canvas: OverlayCanvas,
    /// `None` = redraw needed (new message, or window / picture changed).
    drawn_for: Option<Layout>,
}

impl OnScreenStatus {
    /// Shows `text` for [`STATUS_DURATION`] from `now` (replaces any current message).
    pub fn show(&mut self, text: impl Into<String>, now: Instant) {
        self.text = text.into();
        self.until = Some(now + STATUS_DURATION);
        self.drawn_for = None;
    }

    /// For a state shown while a key is held (rewind): called every redraw, it only
    /// extends a message already on screen (no canvas redraw) and lingers briefly
    /// after the last call.
    pub fn show_while_held(&mut self, text: &str, now: Instant) {
        let on_screen = self.text == text && self.until.is_some_and(|until| now < until);
        if !on_screen {
            self.show(text, now);
        }
        self.until = Some(now + HELD_LINGER);
    }

    /// The message on screen at `now` (`None` once expired), for a composed overlay
    /// (status + achievement layers) that draws it with [`draw_status`].
    pub fn current(&mut self, now: Instant) -> Option<&str> {
        if self.until.is_some_and(|until| now >= until) {
            self.until = None;
        }
        self.until.map(|_| self.text.as_str())
    }

    /// The overlay to blend at `now` (`None` once the message expired), drawn for an
    /// `output`-sized window with the game picture at `game`.
    pub fn overlay(
        &mut self,
        output: (u32, u32),
        game: ViewportRect,
        now: Instant,
    ) -> Option<&OverlayCanvas> {
        self.current(now)?;
        let rect = picture_rect(game);
        let layout = (output, rect);
        if self.drawn_for != Some(layout) {
            self.canvas.reset(output.0, output.1);
            draw_status(&mut self.canvas, rect, &self.text, text_scale(output.1));
            self.drawn_for = Some(layout);
        }
        Some(&self.canvas)
    }
}

/// Font pixel size for an output `height`: ×1 only on tiny windows, ×2 for the
/// default 3× window up to 1440 px, ×3 above (4K / large fullscreen).
pub(crate) fn text_scale(height: u32) -> u32 {
    match height {
        0..360 => 1,
        360..1440 => 2,
        _ => 3,
    }
}

/// The game picture in whole pixels (x, y, w, h).
pub(crate) fn picture_rect(game: ViewportRect) -> [u32; 4] {
    [game.x, game.y, game.width, game.height].map(|v| v.max(0.0) as u32)
}

/// Draws the box + `text` at the bottom-left of `game` (x, y, w, h); skipped when
/// the picture is too small to hold it.
pub(crate) fn draw_status(canvas: &mut OverlayCanvas, game: [u32; 4], text: &str, scale: u32) {
    let [gx, gy, gw, gh] = game;
    let (text_w, text_h) = text_size(text, scale);
    let (box_w, box_h) = (text_w + 2 * PADDING * scale, text_h + 2 * PADDING * scale);
    let margin = MARGIN * scale;
    if box_w + margin > gw || box_h + margin > gh {
        return;
    }
    let (x0, y0) = (gx + margin, gy + gh - box_h - margin);
    canvas.fill_rect(x0, y0, box_w, box_h, BOX_RGBA);
    canvas.draw_text(
        x0 + PADDING * scale,
        y0 + PADDING * scale,
        text,
        scale,
        TEXT_RGBA,
    );
}

#[cfg(test)]
#[path = "on_screen_status_tests.rs"]
mod tests;
