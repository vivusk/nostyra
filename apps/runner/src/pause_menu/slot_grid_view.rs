//! Paints the 2×5 save-state grid: per cell the thumbnail (aspect kept, centred)
//! or "Empty", the highlighted cursor cell, and the save date / time below.

use video_renderer::{text_size, OverlayCanvas};

use super::menu_types::{SlotMode, GRID_COLUMNS, SLOT_COUNT};
use super::pause_menu_state::PauseMenuState;
use super::pause_menu_view::{fit_text, HIGHLIGHT_RGBA, MUTED_RGBA, TEXT_RGBA};
use super::slot_thumbnail_cache::SlotThumbnailCache;

const CELL_RGBA: [u8; 4] = [6, 6, 10, 255];
const ROWS: u32 = SLOT_COUNT.div_ceil(GRID_COLUMNS) as u32;

/// Grid geometry for an `inner_width` wide area at text `scale`.
pub(super) struct GridLayout {
    cell_w: u32,
    thumb_h: u32,
    gap: u32,
    /// Scale of the date / time lines (one step smaller than the menu text).
    small: u32,
}

impl GridLayout {
    pub(super) fn new(inner_width: u32, scale: u32) -> Self {
        let gap = 4 * scale;
        let columns = GRID_COLUMNS as u32;
        let cell_w = inner_width.saturating_sub((columns - 1) * gap) / columns;
        Self {
            cell_w,
            thumb_h: cell_w * 7 / 8, // SNES 256×224
            gap,
            small: scale.saturating_sub(1).max(1),
        }
    }

    fn row_height(&self) -> u32 {
        self.thumb_h + 2 * self.small_line() + self.gap
    }

    fn small_line(&self) -> u32 {
        9 * self.small
    }

    pub(super) fn height(&self) -> u32 {
        ROWS * self.row_height()
    }
}

/// Paints the grid with its top-left at (x, y).
pub(super) fn paint_grid(
    canvas: &mut OverlayCanvas,
    (x, y): (u32, u32),
    layout: &GridLayout,
    state: &PauseMenuState,
    mode: SlotMode,
    thumbnails: &mut SlotThumbnailCache,
) {
    let GridLayout {
        cell_w,
        thumb_h,
        gap,
        small,
    } = *layout;
    for (index, slot) in state.slots().iter().enumerate() {
        let (col, row) = ((index % GRID_COLUMNS) as u32, (index / GRID_COLUMNS) as u32);
        let cx = x + col * (cell_w + gap);
        let cy = y + row * layout.row_height();
        if index == state.slot_cursor() {
            let border = small;
            canvas.fill_rect(
                cx - border,
                cy - border,
                cell_w + 2 * border,
                thumb_h + 2 * border,
                HIGHLIGHT_RGBA,
            );
        }
        canvas.fill_rect(cx, cy, cell_w, thumb_h, CELL_RGBA);
        let enabled = mode == SlotMode::Save || slot.used;
        let color = if enabled { TEXT_RGBA } else { MUTED_RGBA };
        match thumbnails.get(index) {
            Some(thumb) => {
                if let Some(dst) = fit_rect((thumb.width, thumb.height), [cx, cy, cell_w, thumb_h])
                {
                    canvas.blit_rgba_scaled(&thumb.rgba, (thumb.width, thumb.height), dst);
                }
            }
            None => {
                let label = fit_text(if slot.used { "No image" } else { "Empty" }, cell_w, small);
                let (w, h) = text_size(&label, small);
                let (tx, ty) = (
                    cx + cell_w.saturating_sub(w) / 2,
                    cy + thumb_h.saturating_sub(h) / 2,
                );
                canvas.draw_text(tx, ty, &label, small, MUTED_RGBA);
            }
        }
        let (date, time) = slot
            .saved_at
            .as_deref()
            .and_then(|at| at.split_once(' '))
            .unwrap_or(("", ""));
        let lines = [format!("{index} {date}"), time.to_string()];
        for (line_no, line) in lines.iter().enumerate() {
            let ty = cy + thumb_h + small * 2 + line_no as u32 * layout.small_line();
            canvas.draw_text(cx, ty, &fit_text(line, cell_w, small), small, color);
        }
    }
}

/// Largest rectangle with the source aspect inside `cell` (x, y, w, h), centred;
/// `None` for empty sizes.
pub(super) fn fit_rect(src: (u32, u32), cell: [u32; 4]) -> Option<[u32; 4]> {
    let (sw, sh) = (u64::from(src.0), u64::from(src.1));
    let [x, y, w, h] = cell;
    if sw == 0 || sh == 0 || w == 0 || h == 0 {
        return None;
    }
    let (fw, fh) = if u64::from(w) * sh <= u64::from(h) * sw {
        (u64::from(w), (u64::from(w) * sh / sw).max(1))
    } else {
        ((u64::from(h) * sw / sh).max(1), u64::from(h))
    };
    let (fw, fh) = (fw as u32, fh as u32);
    Some([x + (w - fw) / 2, y + (h - fh) / 2, fw, fh])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_rect_keeps_aspect_and_clamps() {
        assert_eq!(
            fit_rect((256, 224), [0, 0, 128, 112]),
            Some([0, 0, 128, 112])
        );
        // Wide source in a square cell: full width, centred vertically.
        assert_eq!(
            fit_rect((200, 100), [10, 10, 100, 100]),
            Some([10, 35, 100, 50])
        );
        // Tall source: full height, centred horizontally.
        assert_eq!(
            fit_rect((100, 200), [0, 0, 100, 100]),
            Some([25, 0, 50, 100])
        );
        // Extreme aspect never collapses to zero; empty sizes are skipped.
        assert_eq!(fit_rect((10_000, 1), [0, 0, 10, 10]), Some([0, 4, 10, 1]));
        assert_eq!(fit_rect((0, 224), [0, 0, 10, 10]), None);
        assert_eq!(fit_rect((256, 224), [0, 0, 0, 10]), None);
    }
}
