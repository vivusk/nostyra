//! On-screen status: placement, tiny pictures, expiry and redraw-on-change.

use super::*;

const GAME: ViewportRect = ViewportRect {
    x: 100.0,
    y: 0.0,
    width: 512.0,
    height: 448.0,
};

fn alpha_at(canvas: &OverlayCanvas, x: u32, y: u32) -> u8 {
    canvas.pixels()[((y * canvas.size().0 + x) * 4 + 3) as usize]
}

#[test]
fn draws_box_and_text_bottom_left_of_the_game_picture() {
    let mut status = OnScreenStatus::default();
    let now = Instant::now();
    status.show("SLOT 3 · SAVED", now);
    let canvas = status.overlay((712, 448), GAME, now).unwrap();
    assert_eq!(canvas.size(), (712, 448));
    let margin = MARGIN * 2;
    assert_eq!(alpha_at(canvas, 0, 447), 0, "left bar untouched");
    assert_eq!(
        alpha_at(canvas, 100 + 300, 10),
        0,
        "top of picture untouched"
    );
    assert_eq!(
        alpha_at(canvas, 100 + margin, 447 - margin),
        BOX_RGBA[3],
        "box corner"
    );
    let text_px = canvas
        .pixels()
        .chunks(4)
        .filter(|px| **px == TEXT_RGBA)
        .count();
    assert!(text_px > 0, "some text pixels");
}

#[test]
fn tiny_pictures_get_no_box() {
    let mut status = OnScreenStatus::default();
    let now = Instant::now();
    status.show("SAVED", now);
    let tiny = ViewportRect {
        x: 0.0,
        y: 0.0,
        width: 8.0,
        height: 8.0,
    };
    let canvas = status.overlay((8, 8), tiny, now).unwrap();
    assert!(canvas.pixels().iter().all(|&c| c == 0));
}

#[test]
fn message_expires_and_redraws_only_on_change() {
    let mut status = OnScreenStatus::default();
    let start = Instant::now();
    assert!(
        status.overlay((712, 448), GAME, start).is_none(),
        "no message yet"
    );
    status.show("SAVED", start);
    let first = status.overlay((712, 448), GAME, start).unwrap().version();
    let same = status.overlay((712, 448), GAME, start).unwrap().version();
    assert_eq!(first, same, "unchanged layout: no redraw/upload");
    let resized = status.overlay((800, 448), GAME, start).unwrap().version();
    assert_ne!(first, resized);
    assert!(status
        .overlay((800, 448), GAME, start + STATUS_DURATION)
        .is_none());
}

#[test]
fn text_scale_follows_output_height() {
    assert_eq!(text_scale(224), 1);
    assert_eq!(text_scale(672), 2);
    assert_eq!(text_scale(1440), 3);
}
