//! One achievement row of the menu page: bold title (dimmer while locked) with the
//! points right-aligned, then the description with the measured progress
//! right-aligned; plus the badge painter (grey square without a badge cache).

use video_renderer::{FontFace, OverlayCanvas, TtfText};

use super::super::achievement_list_page::ListRow;
use super::super::pause_menu_view::{MUTED_RGBA, TEXT_RGBA};
use crate::achievement_overlay::{BadgeCache, BadgeKey};

/// Locked titles: dimmer than unlocked ones, still readable on the highlight.
const LOCKED_TITLE_RGBA: [u8; 4] = [196, 200, 210, 255];

/// Text sizes of the page: title `px`, `small` for the rest, and their line heights.
pub struct RowFonts {
    pub px: u32,
    pub small: u32,
    pub line: u32,
    pub small_line: u32,
}

impl RowFonts {
    pub fn new(text: &TtfText, px: u32) -> Self {
        let small = px * 4 / 5;
        Self {
            px,
            small,
            line: text.line_metrics(px).1,
            small_line: text.line_metrics(small).1,
        }
    }
}

/// Where a row's text goes: left edge, top, width.
pub struct RowCell {
    pub x: u32,
    pub y: u32,
    pub width: u32,
}

/// Title + points on the first line, description + progress on the second.
pub fn paint_row(
    canvas: &mut OverlayCanvas,
    text: &mut TtfText,
    cell: RowCell,
    row: &ListRow,
    fonts: &RowFonts,
) {
    let (bold, regular) = ((FontFace::Bold, fonts.px), (FontFace::Regular, fonts.small));
    let (x, right, gap) = (cell.x as i32, cell.x + cell.width, fonts.px / 2);
    let first = cell.y as i32;
    let points = format!("{} pts", row.points);
    let points_w = text.measure(FontFace::Regular, fonts.small, &points);
    let title_w = cell.width.saturating_sub(points_w + gap);
    let title = text.ellipsize(FontFace::Bold, fonts.px, &row.title, title_w);
    let color = if row.unlocked {
        TEXT_RGBA
    } else {
        LOCKED_TITLE_RGBA
    };
    text.draw(canvas, (x, first), bold, &title, color);
    text.draw(
        canvas,
        ((right - points_w) as i32, first),
        regular,
        &points,
        MUTED_RGBA,
    );
    let second = (cell.y + fonts.line) as i32;
    let progress_w = text.measure(FontFace::Regular, fonts.small, &row.progress);
    let desc_w = cell.width.saturating_sub(progress_w + gap);
    let description = text.ellipsize(FontFace::Regular, fonts.small, &row.description, desc_w);
    text.draw(canvas, (x, second), regular, &description, TEXT_RGBA);
    let progress_x = (right - progress_w) as i32;
    text.draw(
        canvas,
        (progress_x, second),
        regular,
        &row.progress,
        MUTED_RGBA,
    );
}

/// `key` scaled into `rect` from the cache, or a grey square without one.
pub fn draw_badge(
    canvas: &mut OverlayCanvas,
    badges: &mut Option<&mut BadgeCache>,
    key: Option<&BadgeKey>,
    rect: [u32; 4],
) {
    match badges.as_deref_mut() {
        Some(badges) => badges.draw(canvas, key, rect),
        None => canvas.fill_rect(rect[0], rect[1], rect[2], rect[3], MUTED_RGBA),
    }
}
