//! Paints the persistent indicators at the bottom-right of the game picture (the
//! status line keeps the bottom-left): leaderboard tracker boxes stacked from the
//! bottom, the challenge badge row above them, the progress indicator on top.

use video_renderer::{FontFace, OverlayCanvas, TtfText};

use super::badge_cache::BadgeCache;
use super::tracker_indicators::TrackerIndicators;
use super::{BOX_RGBA, TEXT_RGBA};

/// Draws `indicators` inside `game` (x, y, w, h) at text size `px`; stops when the
/// stack would leave the picture.
pub fn draw_indicators(
    canvas: &mut OverlayCanvas,
    (text, badges): (&mut TtfText, &mut BadgeCache),
    game: [u32; 4],
    indicators: &TrackerIndicators,
    px: u32,
) {
    let [gx, gy, gw, gh] = game;
    let (pad, margin) = (px / 3, px / 2);
    let right = gx + gw.saturating_sub(margin);
    let max_text = gw.saturating_sub(2 * margin + 2 * pad);
    let line_h = text.line_metrics(px).1;
    let mut bottom = gy + gh.saturating_sub(margin);
    let fits = |height: u32, bottom: u32| bottom >= gy + height + margin;
    for display in indicators.trackers() {
        let box_h = line_h + 2 * pad;
        let value = text.ellipsize(FontFace::Bold, px, display, max_text);
        let box_w = text.measure(FontFace::Bold, px, &value) + 2 * pad;
        if !fits(box_h, bottom) || box_w > gw {
            return;
        }
        let (x, y) = (right - box_w, bottom - box_h);
        canvas.fill_rect(x, y, box_w, box_h, BOX_RGBA);
        let face = (FontFace::Bold, px);
        text.draw(
            canvas,
            ((x + pad) as i32, (y + pad) as i32),
            face,
            &value,
            TEXT_RGBA,
        );
        bottom = y - pad;
    }
    let size = 2 * px;
    let challenges: Vec<_> = indicators.challenges().collect();
    if !challenges.is_empty() && fits(size, bottom) {
        let mut x = right;
        for badge in challenges {
            if x < gx + size + margin {
                break;
            }
            x -= size;
            badges.draw(canvas, Some(badge), [x, bottom - size, size, size]);
            x -= pad;
        }
        bottom -= size + pad;
    }
    let Some(progress) = indicators.progress() else {
        return;
    };
    let icon = line_h;
    let label = text.ellipsize(FontFace::Regular, px, &progress.text, max_text / 2);
    let box_w = icon + 3 * pad + text.measure(FontFace::Regular, px, &label);
    let box_h = line_h + 2 * pad;
    if !fits(box_h, bottom) || box_w > gw {
        return;
    }
    let (x, y) = (right - box_w, bottom - box_h);
    canvas.fill_rect(x, y, box_w, box_h, BOX_RGBA);
    badges.draw(
        canvas,
        Some(&progress.badge),
        [x + pad, y + pad, icon, icon],
    );
    let text_x = (x + 2 * pad + icon) as i32;
    let face = (FontFace::Regular, px);
    text.draw(canvas, (text_x, (y + pad) as i32), face, &label, TEXT_RGBA);
}
