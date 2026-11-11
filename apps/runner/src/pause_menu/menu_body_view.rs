//! Rows (or the slot grid) of the current menu screen, split from
//! `pause_menu_view.rs`.

use super::menu_labels::{option_label, option_value};
use super::menu_option_values::OPTION_ITEMS;
use super::menu_rows_painter::Rows;
use super::menu_types::{Screen, SlotMode};
use super::pause_menu_state::PauseMenuState;
use super::pause_menu_view::MenuMetrics;
use super::slot_grid_view::{paint_grid, GridLayout};
use super::slot_thumbnail_cache::SlotThumbnailCache;

/// Height of the rows or the grid of the current screen.
pub(super) fn body_height(state: &PauseMenuState, m: &MenuMetrics, grid: &GridLayout) -> u32 {
    let rows = match state.screen() {
        // Every root row, incl. "Reset game", stays visible under a question.
        Screen::Root | Screen::ConfirmQuit | Screen::ConfirmReset => state.root_items().len(),
        Screen::Discs => state.discs().labels.len(),
        Screen::Options => OPTION_ITEMS.len(),
        Screen::Slots(_) | Screen::ConfirmOverwrite(_) => return grid.height(),
        Screen::Achievements => 0, // its own page
    };
    rows as u32 * m.row_h
}

/// Rows (or the grid) of the current screen from `body_y` down.
pub(super) fn paint_body(
    rows: &mut Rows,
    body_y: u32,
    state: &PauseMenuState,
    (grid, thumbnails): (&GridLayout, &mut SlotThumbnailCache),
) {
    let row_h = rows.m.row_h;
    let row_y = |index: usize| body_y + index as u32 * row_h;
    match state.screen() {
        Screen::Root | Screen::ConfirmQuit | Screen::ConfirmReset => {
            for (index, item) in state.root_items().iter().enumerate() {
                let (label, trailing, enabled) = state.root_row(*item);
                let selected = index == state.root_cursor();
                rows.row(row_y(index), &label, trailing, selected, enabled);
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
            let at = (rows.left, body_y);
            paint_grid(
                rows.canvas,
                rows.text,
                at,
                (grid, rows.m),
                state,
                mode,
                thumbnails,
            );
        }
        Screen::ConfirmOverwrite(_) => {
            let (at, mode) = ((rows.left, body_y), SlotMode::Save);
            paint_grid(
                rows.canvas,
                rows.text,
                at,
                (grid, rows.m),
                state,
                mode,
                thumbnails,
            );
        }
        Screen::Achievements => {}
    }
}
