//! Paints the visible popup: a translucent box at the top centre of the window with
//! the icon (badge / game icon, grey square while it downloads) on the left, then a
//! small gold header, the bold title and up to two lines of detail.

use video_renderer::{FontFace, OverlayCanvas, TtfText};

use super::badge_cache::BadgeCache;
use super::popup_queue::Popup;
use super::{ACCENT_RGBA, BOX_RGBA, TEXT_RGBA};

/// Widest popup, in text pixel sizes (keeps a readable column on wide windows).
const MAX_WIDTH_PX: u32 = 34;

/// Draws `popup` for an `output`-sized window at text size `px`; skipped when the
/// window cannot hold it.
pub fn draw_popup(
    canvas: &mut OverlayCanvas,
    (text, badges): (&mut TtfText, &mut BadgeCache),
    output: (u32, u32),
    popup: &Popup,
    px: u32,
) {
    let (pad, small, detail_px) = (px / 2, px * 4 / 5, px * 7 / 8);
    let icon = if popup.icon.is_some() { 3 * px } else { 0 };
    let icon_gap = if icon > 0 { pad } else { 0 };
    let max_width = output.0.saturating_sub(2 * pad).min(MAX_WIDTH_PX * px);
    let text_max = max_width.saturating_sub(icon + icon_gap + 2 * pad);
    if text_max < 4 * px {
        return;
    }
    let header = text.ellipsize(FontFace::Regular, small, &popup.header, text_max);
    let title = text.ellipsize(FontFace::Bold, px, &popup.title, text_max);
    let detail = text.wrap(FontFace::Regular, detail_px, &popup.detail, text_max, 2);
    let text_width = [
        text.measure(FontFace::Regular, small, &header),
        text.measure(FontFace::Bold, px, &title),
    ]
    .into_iter()
    .chain(
        detail
            .iter()
            .map(|line| text.measure(FontFace::Regular, detail_px, line)),
    )
    .max()
    .unwrap_or(0);
    let (small_h, title_h, detail_h) = (
        text.line_metrics(small).1,
        text.line_metrics(px).1,
        text.line_metrics(detail_px).1,
    );
    let content_h = small_h + title_h + detail.len() as u32 * detail_h;
    let box_w = 2 * pad + icon + icon_gap + text_width;
    let box_h = 2 * pad + content_h.max(icon);
    if box_h + pad > output.1 {
        return;
    }
    let (x, y) = ((output.0 - box_w) / 2, pad);
    canvas.fill_rect(x, y, box_w, box_h, BOX_RGBA);
    if icon > 0 {
        let icon_y = y + (box_h - icon) / 2;
        badges.draw(canvas, popup.icon.as_ref(), [x + pad, icon_y, icon, icon]);
    }
    let left = (x + pad + icon + icon_gap) as i32;
    let mut line_y = (y + (box_h - content_h) / 2) as i32;
    text.draw(
        canvas,
        (left, line_y),
        (FontFace::Regular, small),
        &header,
        ACCENT_RGBA,
    );
    line_y += small_h as i32;
    text.draw(
        canvas,
        (left, line_y),
        (FontFace::Bold, px),
        &title,
        TEXT_RGBA,
    );
    line_y += title_h as i32;
    for line in &detail {
        let face = (FontFace::Regular, detail_px);
        text.draw(canvas, (left, line_y), face, line, TEXT_RGBA);
        line_y += detail_h as i32;
    }
}
