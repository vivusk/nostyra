//! CPU conversion of core frames (RGB565 / XRGB8888 / 0RGB1555 / RGBA8888 readback)
//! to tightly packed RGBA8, top row first. SNES frames are ≤ 512×478, so a scalar loop
//! costs well under a millisecond and keeps the shader format-agnostic.

use libretro_host::{PixelFormat, VideoFrame};

/// Expands an n-bit channel to 8 bits by replicating its top bits (0→0, max→255).
#[inline]
fn expand5(v: u16) -> u8 {
    ((v << 3) | (v >> 2)) as u8
}

#[inline]
fn expand6(v: u16) -> u8 {
    ((v << 2) | (v >> 4)) as u8
}

/// Converts `frame` into `out` (resized to `width * height * 4`; reuses capacity,
/// so no allocation once the largest frame size has been seen).
pub fn convert_to_rgba(frame: &VideoFrame<'_>, out: &mut Vec<u8>) {
    let (w, h) = (frame.width as usize, frame.height as usize);
    out.resize(w * h * 4, 0);
    for (y, dst_row) in out.chunks_exact_mut(w * 4).enumerate().take(h) {
        let src = frame.row(y as u32);
        match frame.format {
            PixelFormat::Rgb565 => {
                for (dst, px) in dst_row
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(src.as_chunks::<2>().0)
                {
                    let p = u16::from_le_bytes(*px);
                    *dst = [
                        expand5((p >> 11) & 0x1F),
                        expand6((p >> 5) & 0x3F),
                        expand5(p & 0x1F),
                        0xFF,
                    ];
                }
            }
            PixelFormat::Rgb1555 => {
                for (dst, px) in dst_row
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(src.as_chunks::<2>().0)
                {
                    let p = u16::from_le_bytes(*px);
                    *dst = [
                        expand5((p >> 10) & 0x1F),
                        expand5((p >> 5) & 0x1F),
                        expand5(p & 0x1F),
                        0xFF,
                    ];
                }
            }
            PixelFormat::Xrgb8888 => {
                // Little-endian 0x00RRGGBB → bytes [B, G, R, X].
                for (dst, px) in dst_row
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(src.as_chunks::<4>().0)
                {
                    *dst = [px[2], px[1], px[0], 0xFF];
                }
            }
            PixelFormat::Rgba8888 => {
                // Hardware readback: bytes already R, G, B; alpha is whatever the
                // core's blender left, so force it opaque (PNG thumbnails, overlays).
                for (dst, px) in dst_row
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(src.as_chunks::<4>().0)
                {
                    *dst = [px[0], px[1], px[2], 0xFF];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(format: PixelFormat, data: &[u8], width: u32, pitch: usize) -> Vec<u8> {
        let height = 1;
        let frame = VideoFrame {
            data,
            width,
            height,
            pitch,
            format,
            bottom_up: false,
        };
        let mut out = Vec::new();
        convert_to_rgba(&frame, &mut out);
        out
    }

    #[test]
    fn rgb565_primaries_and_extremes() {
        // red F800, green 07E0, blue 001F, white FFFF (little-endian bytes).
        let data = [0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00, 0xFF, 0xFF];
        let out = convert(PixelFormat::Rgb565, &data, 4, 8);
        assert_eq!(
            out,
            [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]
        );
    }

    #[test]
    fn rgb1555_mid_values() {
        // 0RGB1555: r=16, g=8, b=31 → 0x4000 | 0x0100 | 0x001F = 0x411F.
        let out = convert(PixelFormat::Rgb1555, &[0x1F, 0x41], 1, 2);
        assert_eq!(out, [expand5(16), expand5(8), 255, 255]);
    }

    #[test]
    fn xrgb8888_ignores_padding_and_x_byte() {
        // One pixel 0x7F123456 + 4 bytes pitch padding.
        let data = [0x56, 0x34, 0x12, 0x7F, 9, 9, 9, 9];
        assert_eq!(
            convert(PixelFormat::Xrgb8888, &data, 1, 8),
            [0x12, 0x34, 0x56, 0xFF]
        );
    }

    #[test]
    fn rgba8888_bottom_up_rows_are_flipped_and_alpha_forced_opaque() {
        // Stored bottom row first: row 0 blue (alpha 0), row 1 red (alpha 0x40).
        let data = [0, 0, 255, 0, 255, 0, 0, 0x40];
        let frame = VideoFrame {
            data: &data,
            width: 1,
            height: 2,
            pitch: 4,
            format: PixelFormat::Rgba8888,
            bottom_up: true,
        };
        let mut out = Vec::new();
        convert_to_rgba(&frame, &mut out);
        assert_eq!(out, [255, 0, 0, 255, 0, 0, 255, 255]);
    }
}
