//! CPU-side RGBA8 canvas for the overlay layer (status text, later the pause menu
//! and save-state thumbnails). Pure CPU so drawing is unit-testable; the presenter
//! uploads it only when its `version` changes and blends it over the shaded picture.
//! Colors are straight (non-premultiplied) alpha, gamma-encoded like core frames.

use crate::bitmap_font_5x7::{glyph, GLYPH_ADVANCE, GLYPH_HEIGHT, GLYPH_WIDTH};

/// RGBA8 image, transparent where nothing is drawn.
#[derive(Debug, Default)]
pub struct OverlayCanvas {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    /// Bumped on every change; lets the GPU layer skip unchanged uploads.
    version: u64,
}

/// Pixel size of `text` in the 5×7 font at `scale` (no trailing spacing column).
pub fn text_size(text: &str, scale: u32) -> (u32, u32) {
    let chars = text.chars().count() as u32;
    let width = (chars * GLYPH_ADVANCE).saturating_sub(1);
    (width * scale, GLYPH_HEIGHT * scale)
}

impl OverlayCanvas {
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Tightly packed RGBA8 rows.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// Resizes to `width`×`height` (contents cleared) or, at the same size, clears.
    pub fn reset(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.pixels.clear();
        self.pixels.resize((width * height * 4) as usize, 0);
        self.version += 1;
    }

    /// Writes `rgba` over the rectangle (clipped to the canvas; no blending).
    pub fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, rgba: [u8; 4]) {
        let x_end = x.saturating_add(width).min(self.width);
        let y_end = y.saturating_add(height).min(self.height);
        for row in y..y_end {
            for col in x..x_end {
                self.put(col, row, rgba);
            }
        }
        self.version += 1;
    }

    /// Nearest-neighbour copy of an RGBA8 image (`src_size`, tightly packed) into
    /// the `dst` rectangle (x, y, width, height), clipped to the canvas.
    pub fn blit_rgba_scaled(&mut self, src: &[u8], src_size: (u32, u32), dst: [u32; 4]) {
        let (src_w, src_h) = src_size;
        let [x, y, width, height] = dst;
        if src_w == 0 || src_h == 0 || src.len() < (src_w * src_h * 4) as usize {
            return;
        }
        let x_end = x.saturating_add(width).min(self.width);
        let y_end = y.saturating_add(height).min(self.height);
        for row in y..y_end {
            let sy = u64::from(row - y) * u64::from(src_h) / u64::from(height);
            for col in x..x_end {
                let sx = u64::from(col - x) * u64::from(src_w) / u64::from(width);
                let i = ((sy * u64::from(src_w) + sx) * 4) as usize;
                let mut px = [0; 4];
                px.copy_from_slice(&src[i..i + 4]);
                self.put(col, row, px);
            }
        }
        self.version += 1;
    }

    /// Draws `text` with its top-left at (x, y), each font pixel `scale`×`scale`.
    pub fn draw_text(&mut self, x: u32, y: u32, text: &str, scale: u32, rgba: [u8; 4]) {
        for (index, c) in text.chars().enumerate() {
            let gx = x + index as u32 * GLYPH_ADVANCE * scale;
            for (row, bits) in glyph(c).iter().enumerate() {
                for col in (0..GLYPH_WIDTH).filter(|col| bits >> (GLYPH_WIDTH - 1 - col) & 1 == 1) {
                    let px = gx + col * scale;
                    let py = y + row as u32 * scale;
                    self.fill_rect(px, py, scale, scale, rgba);
                }
            }
        }
        self.version += 1;
    }

    /// Blends `rgba` through an 8-bit coverage mask (`mask_width` columns, rows packed;
    /// a rasterized glyph) with its top-left at (x, y), clipped to the canvas.
    /// Straight-alpha source-over, so text reads correctly over translucent boxes.
    pub fn blend_mask(
        &mut self,
        (x, y): (i32, i32),
        mask: &[u8],
        mask_width: usize,
        rgba: [u8; 4],
    ) {
        if mask_width == 0 {
            return;
        }
        for (row, coverage_row) in mask.chunks(mask_width).enumerate() {
            let py = y + row as i32;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            for (col, &coverage) in coverage_row.iter().enumerate() {
                let px = x + col as i32;
                if px < 0 || px >= self.width as i32 || coverage == 0 {
                    continue;
                }
                let i = ((py as u32 * self.width + px as u32) * 4) as usize;
                blend_over(&mut self.pixels[i..i + 4], coverage, rgba);
            }
        }
        self.version += 1;
    }

    fn put(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        let i = ((y * self.width + x) * 4) as usize;
        if let Some(px) = self.pixels.get_mut(i..i + 4) {
            px.copy_from_slice(&rgba);
        }
    }
}

/// Straight-alpha source-over of `rgba` scaled by `coverage` onto one RGBA8 pixel.
fn blend_over(dst: &mut [u8], coverage: u8, rgba: [u8; 4]) {
    let src_a = u32::from(rgba[3]) * u32::from(coverage) / 255;
    if src_a == 0 {
        return;
    }
    let dst_a = u32::from(dst[3]) * (255 - src_a) / 255;
    let out_a = src_a + dst_a;
    for c in 0..3 {
        dst[c] = ((u32::from(rgba[c]) * src_a + u32::from(dst[c]) * dst_a) / out_a) as u8;
    }
    dst[3] = out_a as u8;
}

#[cfg(test)]
#[path = "overlay_canvas_tests.rs"]
mod tests;
