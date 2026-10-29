use input_mapping::POINTER_MAX;
use system_catalog::{touch_rect, TouchRect, TouchScreenRule};
use video_renderer::{compute_viewport, ScalingMode, ViewportRect};

use super::*;

const DS: TouchScreenRule = TouchScreenRule::DsBottomScreen;
/// 512×768 picture at (100, 50) in the window.
const RECT: ViewportRect = ViewportRect {
    x: 100.0,
    y: 50.0,
    width: 512.0,
    height: 768.0,
};

fn at(cursor: (f64, f64), pressed: bool, touch: TouchRect) -> PointerState {
    cursor_to_pointer(Some(RECT), Some(cursor), pressed, touch)
}

#[test]
fn corners_and_centre_span_the_pointer_range() {
    let full = TouchRect::FULL;
    let top_left = at((100.0, 50.0), false, full);
    assert_eq!((top_left.x, top_left.y), (-POINTER_MAX, -POINTER_MAX));
    let centre = at((356.0, 434.0), false, full);
    assert_eq!((centre.x, centre.y), (0, 0));
    let bottom_right = at((611.999, 817.999), false, full);
    assert!(bottom_right.x > POINTER_MAX - 70 && bottom_right.y > POINTER_MAX - 70);
    assert!(!centre.offscreen && !centre.pressed);
}

#[test]
fn outside_the_picture_is_offscreen_and_never_pressed() {
    for cursor in [(99.0, 400.0), (612.0, 400.0), (300.0, 49.0), (300.0, 818.0)] {
        assert_eq!(
            at(cursor, true, TouchRect::FULL),
            PointerState::default(),
            "{cursor:?}"
        );
    }
    let none = cursor_to_pointer(None, Some((300.0, 400.0)), true, TouchRect::FULL);
    assert_eq!(none, PointerState::default(), "no picture yet");
    let left = cursor_to_pointer(Some(RECT), None, true, TouchRect::FULL);
    assert_eq!(left, PointerState::default(), "cursor left the window");
}

#[test]
fn press_counts_only_inside_the_touch_rect() {
    let bottom = touch_rect(DS, 256, 384);
    let top_screen = at((356.0, 200.0), true, bottom);
    assert!(
        !top_screen.pressed && !top_screen.offscreen,
        "position still reported"
    );
    assert!(at((356.0, 700.0), true, bottom).pressed);
    assert!(!at((356.0, 700.0), false, bottom).pressed, "button up");
    let right = touch_rect(DS, 512, 192);
    assert!(!at((200.0, 434.0), true, right).pressed, "left screen");
    assert!(at((500.0, 434.0), true, right).pressed);
}

#[test]
fn letterboxed_hidpi_window_maps_physical_px() {
    // 1280×720 logical at scale 2 = 2560×1440 physical; DS top-bottom pillarboxed.
    let rect = compute_viewport((2560, 1440), (256, 384), 2.0 / 3.0, ScalingMode::AspectFit);
    assert_eq!((rect.x, rect.width, rect.height), (800.0, 960.0, 1440.0));
    // DS bottom-screen pixel (64, 48) = frame fraction (0.25, 0.625) (spike basis).
    let cursor = (800.0 + 0.25 * 960.0, 0.625 * 1440.0);
    let touch = touch_rect(DS, 256, 384);
    let state = cursor_to_pointer(Some(rect), Some(cursor), true, touch);
    assert_eq!(
        (state.x, state.y),
        (fraction_to_pointer(0.25), fraction_to_pointer(0.625))
    );
    assert!(state.pressed);
    let bar = cursor_to_pointer(Some(rect), Some((400.0, 1000.0)), true, touch);
    assert!(bar.offscreen, "pillarbox bar");
}

#[test]
fn release_paths_and_pause_menu_gate() {
    let mut mouse = MousePointer {
        rule: DS,
        cursor: None,
        in_window: false,
        left_down: false,
        cursor_hidden: false,
    };
    let size = (256, 384);
    mouse.moved(Some((356.0, 700.0)));
    mouse.button(true, false);
    assert!(mouse.state(Some(RECT), size, false).pressed);
    assert!(!mouse.state(Some(RECT), size, true).pressed, "menu open");
    mouse.button(false, false);
    assert!(!mouse.state(Some(RECT), size, false).pressed, "button up");
    mouse.button(true, true);
    assert!(!mouse.left_down, "press while the menu is open is ignored");
    mouse.button(true, false);
    mouse.moved(None);
    assert!(!mouse.left_down, "cursor left the window");
    mouse.moved(Some((356.0, 700.0)));
    assert!(
        !mouse.state(Some(RECT), size, false).pressed,
        "no press on re-entry"
    );
}
