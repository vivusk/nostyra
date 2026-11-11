//! Paints the pause menu into an overlay canvas at output resolution: the whole
//! picture dimmed (70 %, the RetroAchievements hardcore minimum for a pause overlay),
//! then a centred rounded panel in Noto Sans: a header (screen name as a small accent
//! label over the game title), a divider, the screen's rows (rounded highlight pill
//! with an accent bar on the cursor row, greyed disabled rows, "›" on rows that open a
//! page) or the save-state grid, and a footer for results, hints and the overwrite /
//! reset / quit question. Text size follows the output height and shrinks until the
//! panel fits; a window too small even then only gets the dim.

use video_renderer::{OverlayCanvas, TtfText};

use super::achievement_list_view::paint_achievement_list;
use super::menu_body_view::{body_height, paint_body};
use super::menu_panel_painter::{divider, paint_panel};
use super::menu_rows_painter::Rows;
use super::menu_types::Screen;
use super::pause_menu_state::PauseMenuState;
use super::slot_grid_view::GridLayout;
use super::slot_thumbnail_cache::SlotThumbnailCache;
use crate::achievement_overlay::BadgeCache;

/// 179 / 255 = 70 %: at least what RA hardcore asks of a pause overlay.
const DIM_RGBA: [u8; 4] = [0, 0, 0, 179];
pub(super) const HIGHLIGHT_RGBA: [u8; 4] = [52, 92, 180, 255];
pub(super) const TEXT_RGBA: [u8; 4] = [0xF4, 0xF4, 0xF6, 0xFF];
pub(super) const MUTED_RGBA: [u8; 4] = [130, 134, 146, 255];
/// Smallest text size tried before giving up on the panel.
const MIN_PX: u32 = 11;
/// Panel widths in text sizes: list screens and the wider save-state grid.
const LIST_WIDTH_EM: u32 = 26;
const GRID_WIDTH_EM: u32 = 40;

/// Text sizes and spacing of the panel, all derived from the row text size `px`.
pub(super) struct MenuMetrics {
    pub px: u32,
    pub title_px: u32,
    pub small: u32,
    pub line: u32,
    pub title_line: u32,
    pub small_line: u32,
    /// Panel padding (one text size: the "airy" look).
    pub pad: u32,
    /// Gap around dividers.
    pub gap: u32,
    pub row_h: u32,
    pub radius: u32,
    /// Divider / border / accent bar thickness.
    pub stroke: u32,
}

impl MenuMetrics {
    pub(super) fn new(text: &TtfText, px: u32) -> Self {
        let (title_px, small) = (px * 6 / 5, (px * 4 / 5).max(9));
        let line = text.line_metrics(px).1;
        Self {
            px,
            title_px,
            small,
            line,
            title_line: text.line_metrics(title_px).1,
            small_line: text.line_metrics(small).1,
            pad: px,
            gap: px * 3 / 4,
            row_h: line + px * 3 / 5,
            radius: px / 2,
            stroke: (px / 12).max(1),
        }
    }

    /// Footer height: the hint line or the Yes / No buttons.
    pub(super) fn footer_h(&self) -> u32 {
        self.small_line + self.px / 2
    }

    /// Panel height for a `body` of rows or grid.
    fn panel_height(&self, body: u32) -> u32 {
        let header = self.small_line + self.title_line;
        2 * self.pad + header + 2 * (2 * self.gap + self.stroke) + body + self.footer_h()
    }
}

/// Default row text size for an output `height`.
fn menu_px(height: u32) -> u32 {
    (height / 34).clamp(MIN_PX, 56)
}

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
    let grid_screen = matches!(
        state.screen(),
        Screen::Slots(_) | Screen::ConfirmOverwrite(_)
    );
    let width_em = if grid_screen {
        GRID_WIDTH_EM
    } else {
        LIST_WIDTH_EM
    };
    // Largest text size (from the default down) whose panel fits the window.
    let fitted = (MIN_PX..=menu_px(output.1)).rev().find_map(|px| {
        let m = MenuMetrics::new(text, px);
        let width = output.0.saturating_sub(2 * m.pad).min(width_em * px);
        let inner = width.saturating_sub(2 * m.pad);
        let grid = GridLayout::new(inner, &m);
        let height = m.panel_height(body_height(state, &m, &grid));
        (width >= 15 * px && height + 2 * m.pad <= output.1).then_some((m, width, height, grid))
    });
    let Some((m, width, height, grid)) = fitted else {
        return; // too small for a readable panel: dim only
    };
    let (x, y) = ((output.0 - width) / 2, (output.1 - height) / 2);
    paint_panel(canvas, [x, y, width, height], m.radius, m.stroke);
    let (left, inner) = (x + m.pad, width - 2 * m.pad);
    let mut rows = Rows {
        canvas,
        text,
        left,
        inner,
        m: &m,
    };
    let mut cursor_y = rows.header(y + m.pad, title, state.screen());
    cursor_y += m.gap;
    divider(rows.canvas, (left, cursor_y), inner, m.stroke);
    let body_y = cursor_y + m.stroke + m.gap;
    paint_body(&mut rows, body_y, state, (&grid, thumbnails));
    let footer_y = y + height - m.pad - m.footer_h();
    divider(
        rows.canvas,
        (left, footer_y - m.gap - m.stroke),
        inner,
        m.stroke,
    );
    rows.footer(footer_y, state);
}
