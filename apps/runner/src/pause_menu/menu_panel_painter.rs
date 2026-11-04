//! Shapes shared by the pause-menu pages: the bordered rounded panel, rounded
//! highlight pills and divider lines. Writes opaque-enough pixels only (no blending),
//! so the panel never lets the picture through more than the 70 % dim allows.

use video_renderer::OverlayCanvas;

pub(super) const PANEL_RGBA: [u8; 4] = [16, 18, 26, 244];
const BORDER_RGBA: [u8; 4] = [58, 64, 84, 255];
pub(super) const DIVIDER_RGBA: [u8; 4] = [40, 44, 58, 255];

/// `rect` (x, y, w, h) filled with `rgba`, corners rounded by `radius` pixels.
pub(super) fn fill_rounded_rect(
    canvas: &mut OverlayCanvas,
    rect: [u32; 4],
    radius: u32,
    rgba: [u8; 4],
) {
    let [x, y, w, h] = rect;
    let r = radius.min(w / 2).min(h / 2);
    for row in 0..h {
        // Rows inside a corner band are inset to follow the quarter circle.
        let from_edge = row.min(h - 1 - row);
        let inset = if from_edge < r {
            let dy = r as f32 - from_edge as f32 - 0.5;
            let r = r as f32;
            (r - (r * r - dy * dy).max(0.0).sqrt()).round() as u32
        } else {
            0
        };
        canvas.fill_rect(x + inset, y + row, w.saturating_sub(2 * inset), 1, rgba);
    }
}

/// The page panel: a rounded `rect` with a `border` pixel outline.
pub(super) fn paint_panel(canvas: &mut OverlayCanvas, rect: [u32; 4], radius: u32, border: u32) {
    let [x, y, w, h] = rect;
    fill_rounded_rect(canvas, rect, radius, BORDER_RGBA);
    let inner = [
        x + border,
        y + border,
        w.saturating_sub(2 * border),
        h.saturating_sub(2 * border),
    ];
    fill_rounded_rect(canvas, inner, radius.saturating_sub(border), PANEL_RGBA);
}

/// One horizontal divider, `thickness` pixels tall.
pub(super) fn divider(canvas: &mut OverlayCanvas, (x, y): (u32, u32), width: u32, thickness: u32) {
    canvas.fill_rect(x, y, width, thickness, DIVIDER_RGBA);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_corners_leave_the_corner_pixels_untouched() {
        let mut canvas = OverlayCanvas::default();
        canvas.reset(20, 20);
        fill_rounded_rect(&mut canvas, [0, 0, 20, 20], 6, [255, 0, 0, 255]);
        let px = |x: u32, y: u32| canvas.pixels()[((y * 20 + x) * 4) as usize..][..4].to_vec();
        assert_eq!(px(0, 0), [0, 0, 0, 0], "corner stays clear");
        assert_eq!(px(19, 19), [0, 0, 0, 0], "opposite corner stays clear");
        assert_eq!(px(10, 0), [255, 0, 0, 255], "top edge filled");
        assert_eq!(px(0, 10), [255, 0, 0, 255], "left edge filled");
        // Radius larger than the rect and zero sizes never panic.
        fill_rounded_rect(&mut canvas, [0, 0, 3, 1], 50, [1, 1, 1, 255]);
        fill_rounded_rect(&mut canvas, [0, 0, 0, 0], 4, [1, 1, 1, 255]);
    }
}
