//! In-game HUD lines (bottom-right, newest warning on top): worst ping, rollback
//! frames per second and a RELAY badge (`--netplay-hud on`), plus warnings that
//! show either way: a stalled peer, a desync, a peer that left.

use std::time::Instant;

use video_renderer::{text_size, OverlayCanvas};

use super::{BOX_RGBA, TEXT_RGBA, WARN_RGBA};
use crate::netplay_runtime::NetplayRuntime;

const PADDING: u32 = 3;
const MARGIN: u32 = 4;

/// One HUD line and whether it is a warning.
pub(super) type HudLine = (String, bool);

/// Whether [`hud_lines`] would show anything (no strings built).
pub(super) fn hud_visible(runtime: &NetplayRuntime, hud_on: bool, peer_left: bool) -> bool {
    let hud = runtime.hud();
    hud_on || peer_left || hud.desync_frame.is_some() || hud.stalled
}

pub(super) fn hud_lines(runtime: &NetplayRuntime, hud_on: bool, now: Instant) -> Vec<HudLine> {
    let hud = runtime.hud();
    let mut lines = Vec::new();
    if let Some(left) = runtime.peer_left_text(now) {
        lines.push((super::ascii(&left), true));
    }
    if hud.desync_frame.is_some() {
        let text = match runtime.is_host() {
            true => "Desync detected - menu: Resync",
            false => "Desync detected - host can resync",
        };
        lines.push((text.to_owned(), true));
    }
    if hud.stalled {
        lines.push(("Waiting for a player...".to_owned(), true));
    }
    if hud_on {
        let ping = hud
            .ping_ms
            .map_or_else(|| "--".to_owned(), |p| p.to_string());
        let relay = if hud.relayed { "  RELAY" } else { "" };
        lines.push((
            format!("Ping {ping} ms  RB {}/s{relay}", hud.rollback_fps),
            false,
        ));
    }
    lines
}

/// Stacks `lines` from the bottom-right corner of `game` (x, y, w, h) upwards.
pub(super) fn paint(canvas: &mut OverlayCanvas, game: [u32; 4], lines: &[HudLine], scale: u32) {
    let [gx, gy, gw, gh] = game;
    let (pad, margin) = (PADDING * scale, MARGIN * scale);
    let mut bottom = (gy + gh).saturating_sub(margin);
    for (text, warning) in lines.iter().rev() {
        let (w, h) = text_size(text, scale);
        let (box_w, box_h) = (w + 2 * pad, h + 2 * pad);
        if box_w + 2 * margin > gw || bottom < gy + box_h + margin {
            return;
        }
        let (x, y) = (gx + gw - margin - box_w, bottom - box_h);
        canvas.fill_rect(x, y, box_w, box_h, BOX_RGBA);
        let color = if *warning { WARN_RGBA } else { TEXT_RGBA };
        canvas.draw_text(x + pad, y + pad, text, scale, color);
        bottom = y.saturating_sub(pad);
    }
}
