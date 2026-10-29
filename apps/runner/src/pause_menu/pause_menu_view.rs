//! Paints the pause menu into an overlay canvas at output resolution: the whole
//! picture dimmed (70 %, the RetroAchievements hardcore minimum for a pause overlay), a centred panel with the ROM key as title, the screen's
//! rows (highlighted cursor row, greyed disabled rows), and a footer for results,
//! hints and the overwrite / reset / quit question. Text scale follows the OSD rule (×1/×2/×3 by
//! output height); a window too small for the panel only gets the dim.

use video_renderer::{text_size, OverlayCanvas, TtfText};

use super::achievement_list_view::paint_achievement_list;
use super::menu_labels::{footer_hint, option_label, option_value, root_label, screen_title};
use super::menu_option_values::OPTION_ITEMS;
use super::menu_types::{RootItem, Screen, SlotMode};
use super::pause_menu_state::PauseMenuState;
use super::slot_grid_view::{paint_grid, GridLayout};
use super::slot_thumbnail_cache::SlotThumbnailCache;
use crate::achievement_overlay::BadgeCache;
use crate::on_screen_status::text_scale;

/// 179 / 255 = 70 %: at least what RA hardcore asks of a pause overlay.
const DIM_RGBA: [u8; 4] = [0, 0, 0, 179];
const PANEL_RGBA: [u8; 4] = [20, 22, 30, 240];
pub(super) const HIGHLIGHT_RGBA: [u8; 4] = [52, 92, 180, 255];
pub(super) const TEXT_RGBA: [u8; 4] = [0xF4, 0xF4, 0xF6, 0xFF];
pub(super) const MUTED_RGBA: [u8; 4] = [130, 134, 146, 255];
const TITLE_RGBA: [u8; 4] = [255, 204, 102, 255];
/// Panel width cap in font pixels (wider windows keep a readable column).
const MAX_PANEL_WIDTH: u32 = 300;

/// Repaints `canvas` (sized to `output`) for the current menu state; the
/// achievement page draws with `text` and `badges`.
pub fn paint_menu(
    canvas: &mut OverlayCanvas,
    (output, title): ((u32, u32), &str),
    state: &PauseMenuState,
    thumbnails: &mut SlotThumbnailCache,
    (text, badges): (&mut TtfText, Option<&mut BadgeCache>),
) {
    canvas.reset(output.0, output.1);
    canvas.fill_rect(0, 0, output.0, output.1, DIM_RGBA);
    if state.screen() == Screen::Achievements {
        let list = state.achievement_list();
        return paint_achievement_list(canvas, output, list, text, badges);
    }
    let s = text_scale(output.1);
    let (line, pad) = (11 * s, 6 * s);
    let width = output.0.saturating_sub(4 * s).min(MAX_PANEL_WIDTH * s);
    let inner = width.saturating_sub(2 * pad);
    let grid = GridLayout::new(inner, s);
    let body = match state.screen() {
        Screen::Root | Screen::ConfirmQuit | Screen::ConfirmReset => {
            state.root_items().len() as u32 * line // every root row, incl. "Reset game"
        }
        Screen::Discs => state.discs().labels.len() as u32 * line,
        Screen::Achievements => 0, // painted above
        Screen::Options => OPTION_ITEMS.len() as u32 * line,
        Screen::Slots(_) | Screen::ConfirmOverwrite(_) => grid.height(),
    };
    let height = 2 * pad + 3 * line + body + line + line / 2; // footer after a half-line gap
                                                              // Narrower than the overwrite question + Yes / No, or too short: no panel.
    if width < 160 * s || height > output.1 {
        return; // too small for a readable panel: dim only
    }
    let (x, y) = ((output.0 - width) / 2, (output.1 - height) / 2);
    canvas.fill_rect(x, y, width, height, PANEL_RGBA);
    let left = x + pad;
    let mut rows = Rows {
        canvas,
        left,
        inner,
        line,
        s,
    };
    let ascii_title: String = title
        .chars()
        .map(|c| if c.is_ascii() { c } else { '?' })
        .collect();
    rows.text(y + pad, &ascii_title, TITLE_RGBA);
    rows.text(y + pad + line, screen_title(state.screen()), MUTED_RGBA);
    let body_y = y + pad + 3 * line;
    let row_y = |index: usize| body_y + index as u32 * line;
    match state.screen() {
        Screen::Root | Screen::ConfirmQuit | Screen::ConfirmReset => {
            for (index, item) in state.root_items().iter().enumerate() {
                let enabled = *item != RootItem::LoadState || state.load_enabled();
                let selected = index == state.root_cursor();
                rows.row(row_y(index), root_label(*item), None, selected, enabled);
            }
        }
        Screen::Options => {
            for (index, item) in OPTION_ITEMS.iter().enumerate() {
                let (value, enabled) = option_value(*item, state.options());
                let selected = index == state.option_cursor();
                rows.row(row_y(index), option_label(*item), value, selected, enabled);
            }
        }
        Screen::Discs => {
            let discs = state.discs();
            for (index, label) in discs.labels.iter().enumerate() {
                let inserted = (index == discs.current).then(|| "Inserted".to_string());
                let selected = index == state.disc_cursor();
                rows.row(row_y(index), label, inserted, selected, true);
            }
        }
        Screen::Slots(mode) => {
            paint_grid(rows.canvas, (left, body_y), &grid, state, mode, thumbnails)
        }
        Screen::ConfirmOverwrite(_) => {
            let mode = SlotMode::Save;
            paint_grid(rows.canvas, (left, body_y), &grid, state, mode, thumbnails);
        }
        Screen::Achievements => {}
    }
    rows.footer(y + height - pad - line + 2 * s, state);
}

/// Row painter for one panel column.
struct Rows<'a> {
    canvas: &'a mut OverlayCanvas,
    left: u32,
    inner: u32,
    line: u32,
    s: u32,
}

impl Rows<'_> {
    /// One line of `text` at the column's left edge, cut to the column width.
    fn text(&mut self, y: u32, text: &str, color: [u8; 4]) {
        let text = fit_text(text, self.inner, self.s);
        self.canvas.draw_text(self.left, y, &text, self.s, color);
    }

    /// Label left, optional value right-aligned; cursor row highlighted.
    fn row(&mut self, y: u32, label: &str, value: Option<String>, selected: bool, enabled: bool) {
        if selected {
            let half = 3 * self.s;
            self.canvas.fill_rect(
                self.left - half,
                y,
                self.inner + 2 * half,
                self.line,
                HIGHLIGHT_RGBA,
            );
        }
        let color = if enabled { TEXT_RGBA } else { MUTED_RGBA };
        let text_y = y + 2 * self.s;
        self.text(text_y, label, color);
        if let Some(value) = value {
            let value = fit_text(&value, self.inner / 2, self.s);
            let value_x = self.left + self.inner - text_size(&value, self.s).0;
            self.canvas
                .draw_text(value_x, text_y, &value, self.s, color);
        }
    }

    /// Result message, disabled-row reason, overwrite / reset / quit question or a key hint.
    fn footer(&mut self, y: u32, state: &PauseMenuState) {
        let question = match state.screen() {
            Screen::ConfirmOverwrite(slot) => Some(format!("Overwrite slot {slot}?")),
            Screen::ConfirmQuit => Some("Quit game?".to_string()),
            Screen::ConfirmReset => Some("Reset game?".to_string()),
            _ => None,
        };
        if let Some(question) = question {
            self.text(y, &question, TEXT_RGBA);
            let mut x = self.left + text_size(&question, self.s).0 + 4 * self.s;
            for (label, chosen) in [("Yes", state.confirm_yes()), ("No", !state.confirm_yes())] {
                let (w, h) = text_size(label, self.s);
                if chosen {
                    self.canvas.fill_rect(
                        x - self.s,
                        y - self.s,
                        w + 2 * self.s,
                        h + 2 * self.s,
                        HIGHLIGHT_RGBA,
                    );
                }
                self.canvas.draw_text(x, y, label, self.s, TEXT_RGBA);
                x += w + 4 * self.s;
            }
            return;
        }
        self.text(y, footer_hint(state), MUTED_RGBA);
    }
}

/// `text`, cut (with "..") to fit `max_width` pixels at `scale`.
pub(super) fn fit_text(text: &str, max_width: u32, scale: u32) -> String {
    let max_chars = ((max_width / scale.max(1) + 1) / 6) as usize;
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars < 3 {
        return String::new();
    }
    let kept: String = text.chars().take(max_chars.saturating_sub(2)).collect();
    format!("{kept}..")
}
