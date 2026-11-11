//! Netplay layers on the overlay canvas (5×7 font, blended after the shader like
//! the status line): the full-window waiting room over the frozen core until the
//! session runs (and again while resyncing or after it ended), then the HUD at the
//! bottom-right of the picture with the status line at the bottom-left. Repainted
//! only when what it shows changed.

#[cfg(test)]
mod hot_path_alloc_tests;
mod hud_view;
mod waiting_room_view;

use std::time::Instant;

use video_renderer::{OverlayCanvas, ViewportRect};

use crate::netplay_runtime::NetplayRuntime;
use crate::on_screen_status::{draw_status, picture_rect, text_scale};

/// Box / text colours shared with the status line.
const BOX_RGBA: [u8; 4] = [0, 0, 0, 166];
const DIM_RGBA: [u8; 4] = [0, 0, 0, 200];
const TEXT_RGBA: [u8; 4] = [0xF4, 0xF4, 0xF6, 0xFF];
const MUTED_RGBA: [u8; 4] = [0xA8, 0xA8, 0xB0, 0xFF];
const WARN_RGBA: [u8; 4] = [0xFF, 0xC8, 0x40, 0xFF];

/// What a painted canvas showed (repaint when it differs). `Copy` + the status text
/// kept beside it: checking costs no allocation on the redraws that change nothing.
#[derive(Clone, Copy, PartialEq, Eq)]
struct PaintKey {
    output: (u32, u32),
    rect: [u32; 4],
    revision: u64,
    /// HUD: (`--netplay-hud on`, "<nick> left" shown); `None` = waiting room.
    hud: Option<(bool, bool)>,
}

#[derive(Default)]
pub struct NetplayOverlay {
    canvas: OverlayCanvas,
    painted: Option<PaintKey>,
    /// Status line text of the painted HUD (capacity reused).
    painted_status: String,
}

/// The 5×7 font shows ASCII only: other characters become `?`.
pub(crate) fn ascii(text: &str) -> String {
    let printable = |c: char| {
        if c.is_ascii_graphic() || c == ' ' {
            c
        } else {
            '?'
        }
    };
    text.chars().map(printable).collect()
}

impl NetplayOverlay {
    /// Whether the session shows the waiting room (not running yet, resync, end).
    pub fn shows_waiting_room(runtime: &NetplayRuntime) -> bool {
        *runtime.phase() != netplay_session::lobby_state::LobbyPhase::Running
    }

    /// The full-window waiting room for an `output`-sized window.
    pub fn waiting_room(&mut self, output: (u32, u32), runtime: &NetplayRuntime) -> &OverlayCanvas {
        let key = PaintKey {
            output,
            rect: [0; 4],
            revision: runtime.revision(),
            hud: None,
        };
        if self.painted != Some(key) {
            self.canvas.reset(output.0, output.1);
            waiting_room_view::paint(&mut self.canvas, output, runtime);
            self.painted = Some(key);
        }
        &self.canvas
    }

    /// The in-game HUD (+ the status line `status`) over the picture `game`;
    /// `None` when there is nothing to show.
    pub fn hud(
        &mut self,
        (output, game): ((u32, u32), ViewportRect),
        runtime: &NetplayRuntime,
        (hud_on, status): (bool, Option<&str>),
        now: Instant,
    ) -> Option<&OverlayCanvas> {
        let peer_left = runtime.peer_left_shown(now);
        if !hud_view::hud_visible(runtime, hud_on, peer_left) && status.is_none() {
            self.painted = None;
            return None;
        }
        let rect = picture_rect(game);
        let key = PaintKey {
            output,
            rect,
            revision: runtime.revision(), // bumped on every stats / warning change
            hud: Some((hud_on, peer_left)),
        };
        let text = status.unwrap_or("");
        if self.painted != Some(key) || self.painted_status != text {
            // Lines are built on a repaint only (a few times per second at most).
            let lines = hud_view::hud_lines(runtime, hud_on, now);
            self.canvas.reset(output.0, output.1);
            let scale = text_scale(output.1);
            hud_view::paint(&mut self.canvas, rect, &lines, scale);
            if status.is_some() {
                draw_status(&mut self.canvas, rect, text, scale);
            }
            self.painted = Some(key);
            self.painted_status.clear();
            self.painted_status.push_str(text);
        }
        Some(&self.canvas)
    }
}
