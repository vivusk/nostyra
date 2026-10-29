//! Unit tests of `OverlayCanvas` (fill, blit, 5×7 text, coverage blend, reset).

use super::*;

const RED: [u8; 4] = [255, 0, 0, 255];

fn canvas(width: u32, height: u32) -> OverlayCanvas {
    let mut canvas = OverlayCanvas::default();
    canvas.reset(width, height);
    canvas
}

fn at(canvas: &OverlayCanvas, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * canvas.width + x) * 4) as usize;
    canvas.pixels[i..i + 4].try_into().unwrap()
}

#[test]
fn fill_rect_is_clipped_and_bumps_version() {
    let mut c = canvas(4, 3);
    let before = c.version();
    c.fill_rect(2, 1, 10, 10, RED);
    assert!(c.version() > before);
    assert_eq!(at(&c, 1, 1), [0; 4], "left of the rect stays transparent");
    assert_eq!(at(&c, 3, 2), RED, "clipped rect still covers the corner");
    assert_eq!(c.pixels().len(), 4 * 3 * 4);
}

#[test]
fn scaled_blit_doubles_pixels_and_clips() {
    let src = [RED, [0, 255, 0, 255]].concat(); // 2×1: red, green
    let mut c = canvas(5, 2);
    c.blit_rgba_scaled(&src, (2, 1), [1, 0, 4, 2]);
    assert_eq!(at(&c, 0, 0), [0; 4]);
    assert_eq!(at(&c, 1, 0), RED);
    assert_eq!(at(&c, 2, 1), RED);
    assert_eq!(at(&c, 3, 0), [0, 255, 0, 255]);
    assert_eq!(at(&c, 4, 1), [0, 255, 0, 255], "x = 5 is clipped away");
    c.blit_rgba_scaled(&src[..4], (2, 1), [0, 0, 2, 2]); // short source: ignored
    assert_eq!(at(&c, 0, 0), [0; 4]);
}

#[test]
fn text_pixels_match_the_font_at_scale() {
    let mut c = canvas(20, 20);
    c.draw_text(1, 2, "I", 2, RED);
    // 'I' row 0 is 0x0E: columns 1..=3 lit, column 0 blank.
    assert_eq!(at(&c, 1, 2), [0; 4]);
    assert_eq!(at(&c, 3, 2), RED);
    assert_eq!(at(&c, 4, 3), RED, "2×2 font pixel");
    assert_eq!(text_size("SAVED", 2), ((5 * 6 - 1) * 2, 14));
    assert_eq!(text_size("", 3), (0, 21));
}

#[test]
fn reset_clears_and_resizes() {
    let mut c = canvas(2, 2);
    c.fill_rect(0, 0, 2, 2, RED);
    c.reset(3, 1);
    assert_eq!(c.size(), (3, 1));
    assert!(c.pixels().iter().all(|&b| b == 0));
}

#[test]
fn mask_blend_is_source_over_and_clipped() {
    let mut c = canvas(3, 2);
    c.fill_rect(0, 0, 3, 2, [0, 0, 0, 128]);
    // 2×2 mask placed at (2, -1): only its bottom-left pixel lands on the canvas.
    c.blend_mask((2, -1), &[255, 255, 255, 0], 2, [255, 255, 255, 255]);
    assert_eq!(at(&c, 2, 0), [255, 255, 255, 255], "full coverage replaces");
    assert_eq!(at(&c, 1, 0), [0, 0, 0, 128], "outside the mask untouched");
    c.blend_mask((0, 1), &[128], 1, [255, 255, 255, 255]);
    let [r, _, _, a] = at(&c, 0, 1);
    // Straight alpha: colour 255·128 / (128 + 63) ≈ 170 at alpha 191 (≈ 50 % grey composited).
    assert!((165..=175).contains(&r), "half coverage over the box: {r}");
    assert!(
        (185..=195).contains(&a),
        "coverage adds alpha over the box: {a}"
    );
    c.blend_mask((0, 0), &[], 0, RED); // empty mask: no panic
}
