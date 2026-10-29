//! TTF overlay text: pixel bounds, faces, wrap, ellipsis, NFC and missing glyphs.

use super::*;

const WHITE: [u8; 4] = [255, 255, 255, 255];

fn canvas(width: u32, height: u32) -> OverlayCanvas {
    let mut canvas = OverlayCanvas::default();
    canvas.reset(width, height);
    canvas
}

/// Bounding box (x0, y0, x1, y1) of pixels with any alpha.
fn ink_bounds(canvas: &OverlayCanvas) -> Option<(u32, u32, u32, u32)> {
    let (w, _) = canvas.size();
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for (i, px) in canvas.pixels().chunks(4).enumerate() {
        if px[3] > 0 {
            let (x, y) = (i as u32 % w, i as u32 / w);
            bounds = Some(match bounds {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
    }
    bounds
}

#[test]
fn drawn_text_stays_inside_its_measured_box() {
    let mut text = TtfText::default();
    let mut c = canvas(400, 60);
    let face = (FontFace::Regular, 24);
    let width = text.draw(&mut c, (10, 5), face, "Pokémon Привет", WHITE);
    assert_eq!(width, text.measure(FontFace::Regular, 24, "Pokémon Привет"));
    let (ascent, height) = text.line_metrics(24);
    assert!(
        ascent < height && (24..=40).contains(&height),
        "{ascent} {height}"
    );
    let (x0, y0, x1, y1) = ink_bounds(&c).expect("ink");
    assert!(
        x0 >= 10 && x1 <= 10 + width + 2,
        "x {x0}..{x1} in 10..{}",
        10 + width
    );
    assert!(y0 >= 5 && y1 < 5 + height, "y {y0}..{y1} within the line");
}

#[test]
fn bold_is_wider_and_cached_separately() {
    let mut text = TtfText::default();
    let regular = text.measure(FontFace::Regular, 20, "Mastered");
    let glyphs = text.cached_glyphs();
    let bold = text.measure(FontFace::Bold, 20, "Mastered");
    assert!(bold > regular, "bold {bold} > regular {regular}");
    assert!(
        text.cached_glyphs() > glyphs,
        "bold glyphs get their own cache keys"
    );
}

#[test]
fn ellipsis_cuts_to_the_width_and_keeps_short_text() {
    let mut text = TtfText::default();
    let face = FontFace::Regular;
    assert_eq!(text.ellipsize(face, 16, "Short", 500), "Short");
    let long = "A very long achievement description that cannot fit";
    let cut = text.ellipsize(face, 16, long, 120);
    assert!(cut.ends_with('…') && cut.len() < long.len(), "{cut}");
    assert!(text.measure(face, 16, &cut) <= 120);
    assert_eq!(text.ellipsize(face, 16, long, 1), "", "nothing fits");
}

#[test]
fn wrap_fills_lines_and_ellipsizes_the_last() {
    let mut text = TtfText::default();
    let face = FontFace::Regular;
    let long = "Collect every coin in the first world without losing a single life";
    let lines = text.wrap(face, 16, long, 200, 2);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[1].ends_with('…'), "{lines:?}");
    assert!(lines.iter().all(|l| text.measure(face, 16, l) <= 200));
    assert_eq!(text.wrap(face, 16, "Two words", 500, 3), ["Two words"]);
    assert!(text.wrap(face, 16, "   ", 500, 2).is_empty());
}

#[test]
fn decomposed_text_is_normalised_before_drawing() {
    let mut text = TtfText::default();
    let composed = text.measure(FontFace::Regular, 24, "Tiếng Việt");
    let decomposed = text.measure(
        FontFace::Regular,
        24,
        "Tie\u{302}\u{301}ng Vie\u{323}\u{302}t",
    );
    assert_eq!(composed, decomposed);
    assert_eq!(text.ellipsize(FontFace::Regular, 24, "e\u{301}", 500), "é");
}

#[test]
fn missing_glyphs_draw_the_font_box() {
    let mut text = TtfText::default();
    let mut c = canvas(80, 40);
    let width = text.draw(&mut c, (2, 2), (FontFace::Regular, 24), "日", WHITE);
    assert!(width > 0, "the .notdef box has an advance");
    assert!(ink_bounds(&c).is_some(), "box drawn");
}

#[test]
fn digits_are_tabular() {
    let mut text = TtfText::default();
    let one = text.measure(FontFace::Bold, 24, "1111");
    let eight = text.measure(FontFace::Bold, 24, "8888");
    assert_eq!(one, eight);
}
