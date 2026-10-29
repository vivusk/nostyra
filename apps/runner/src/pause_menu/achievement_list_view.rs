//! Paints the "Achievements" page over the dimmed picture: a tall panel with the game
//! header (icon, title, mode and totals), then section labels and achievement rows
//! (`achievement_list_row_view`), the selected row highlighted, and a key hint with
//! the row counter. Unicode text in Noto Sans; badges still downloading draw grey.

use video_renderer::{FontFace, OverlayCanvas, TtfText};

use self::achievement_list_row_view::{draw_badge, paint_row, RowCell, RowFonts};
use super::achievement_list_page::ListEntry;
use super::achievement_list_page_state::AchievementListPageState;
use super::pause_menu_view::{HIGHLIGHT_RGBA, MUTED_RGBA, TEXT_RGBA};
use crate::achievement_overlay::{BadgeCache, ACCENT_RGBA};

mod achievement_list_row_view;

const PANEL_RGBA: [u8; 4] = [20, 22, 30, 240];
const HINT: &str = "Up / Down: scroll   Left / Right: page   Esc / B: back";

/// Text size of the page for an output `height` (a little smaller than popups).
pub fn list_px(height: u32) -> u32 {
    (height / 45).clamp(12, 48)
}

/// Paints `list` for an `output`-sized window (the dim is already drawn).
pub fn paint_achievement_list(
    canvas: &mut OverlayCanvas,
    output: (u32, u32),
    list: &AchievementListPageState,
    text: &mut TtfText,
    mut badges: Option<&mut BadgeCache>,
) {
    let fonts = RowFonts::new(text, list_px(output.1));
    let (px, small, line, small_line) = (fonts.px, fonts.small, fonts.line, fonts.small_line);
    let pad = px / 2;
    let width = output.0.saturating_sub(2 * px).min(50 * px);
    let height = output.1.saturating_sub(2 * px);
    let Some(page) = list
        .page()
        .filter(|_| width >= 16 * px && height >= 8 * line)
    else {
        return; // too small for a readable page: dim only
    };
    let (x, y) = ((output.0 - width) / 2, (output.1 - height) / 2);
    canvas.fill_rect(x, y, width, height, PANEL_RGBA);
    let (left, inner) = (x + pad, width - 2 * pad);
    let (bold, regular) = ((FontFace::Bold, px), (FontFace::Regular, small));
    // Header: game icon, title, mode and totals.
    let header_h = line + small_line;
    let icon_rect = [left, y + pad, header_h, header_h];
    draw_badge(canvas, &mut badges, page.header.icon.as_ref(), icon_rect);
    let (text_x, text_w) = (
        (left + header_h + pad) as i32,
        inner.saturating_sub(header_h + pad),
    );
    let title = text.ellipsize(FontFace::Bold, px, &page.header.title, text_w);
    text.draw(
        canvas,
        (text_x, (y + pad) as i32),
        bold,
        &title,
        ACCENT_RGBA,
    );
    let summary = text.ellipsize(FontFace::Regular, small, &page.header.summary, text_w);
    text.draw(
        canvas,
        (text_x, (y + pad + line) as i32),
        regular,
        &summary,
        MUTED_RGBA,
    );
    // Entries between the header and the hint line.
    let rows_top = y + 3 * pad + header_h;
    let rows_bottom = y + height - 2 * pad - small_line;
    let row_h = line + small_line + pad;
    list.set_visible(((rows_bottom - rows_top) / row_h) as usize);
    if page.entries.is_empty() {
        let empty = "No achievements for this game";
        text.draw(
            canvas,
            (left as i32, rows_top as i32),
            bold,
            empty,
            TEXT_RGBA,
        );
    }
    let mut row_y = rows_top;
    for (index, entry) in page.entries.iter().enumerate().skip(list.top()) {
        match entry {
            ListEntry::Section(label) if row_y + small_line <= rows_bottom => {
                let face = (FontFace::Bold, small);
                text.draw(
                    canvas,
                    (left as i32, row_y as i32),
                    face,
                    label,
                    ACCENT_RGBA,
                );
                row_y += small_line + pad / 2;
            }
            ListEntry::Row(row) if row_y + row_h <= rows_bottom => {
                if index == list.cursor() {
                    canvas.fill_rect(left - pad / 2, row_y, inner + pad, row_h, HIGHLIGHT_RGBA);
                }
                let icon = row_h - pad;
                let icon_rect = [left, row_y + pad / 2, icon, icon];
                draw_badge(canvas, &mut badges, row.badge.as_ref(), icon_rect);
                let width = inner.saturating_sub(icon + pad);
                let cell = RowCell {
                    x: left + icon + pad,
                    y: row_y + pad / 2,
                    width,
                };
                paint_row(canvas, text, cell, row, &fonts);
                row_y += row_h;
            }
            _ => break,
        }
    }
    // Key hint left, "row / rows" right.
    let rows = page.row_indexes();
    let position = rows.iter().position(|&i| i == list.cursor());
    let counter = position.map_or(String::new(), |p| format!("{} / {}", p + 1, rows.len()));
    let hint_y = (y + height - pad - small_line) as i32;
    let counter_w = text.measure(FontFace::Regular, small, &counter);
    let hint_w = inner.saturating_sub(counter_w + pad);
    let hint = text.ellipsize(FontFace::Regular, small, HINT, hint_w);
    text.draw(canvas, (left as i32, hint_y), regular, &hint, MUTED_RGBA);
    let counter_x = (left + inner - counter_w) as i32;
    text.draw(canvas, (counter_x, hint_y), regular, &counter, MUTED_RGBA);
}
