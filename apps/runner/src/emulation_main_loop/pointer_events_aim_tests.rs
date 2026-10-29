//! Wii Remote IR (aiming rule): whole-frame pointer over the picture, edge-clamped
//! offscreen position off it / out of the window, OS cursor hide decision. The DS
//! touch rule keeps its own behaviour (`pointer_events_tests.rs`).

use input_mapping::{fraction_to_pointer, PointerState, POINTER_MAX};
use system_catalog::TouchScreenRule;
use video_renderer::{compute_viewport, ScalingMode, ViewportRect};

use super::super::MousePointer;
use super::*;

const WII: TouchScreenRule = TouchScreenRule::WiiRemoteIr;
/// 640×480 picture at (100, 50) in the window.
const RECT: ViewportRect = ViewportRect {
    x: 100.0,
    y: 50.0,
    width: 640.0,
    height: 480.0,
};

fn aim(cursor: (f64, f64), pressed: bool) -> PointerState {
    aim_to_pointer(Some(RECT), Some(cursor), true, pressed)
}

fn mouse(rule: TouchScreenRule) -> MousePointer {
    MousePointer {
        rule,
        cursor: None,
        in_window: false,
        left_down: false,
        cursor_hidden: false,
    }
}

#[test]
fn over_the_picture_spans_the_whole_frame() {
    let top_left = aim((100.0, 50.0), false);
    assert_eq!((top_left.x, top_left.y), (-POINTER_MAX, -POINTER_MAX));
    assert!(!top_left.offscreen);
    let centre = aim((420.0, 290.0), true);
    assert_eq!((centre.x, centre.y), (0, 0));
    assert!(
        centre.pressed && !centre.offscreen,
        "press anywhere on the frame"
    );
    let near_bottom_right = aim((739.999, 529.999), false);
    assert!(near_bottom_right.x > POINTER_MAX - 120 && near_bottom_right.y > POINTER_MAX - 120);
}

#[test]
fn off_the_picture_parks_at_the_nearest_edge() {
    let quarter = fraction_to_pointer(0.25);
    for (cursor, want) in [
        ((20.0, 170.0), (-POINTER_MAX, quarter)),       // left
        ((900.0, 170.0), (POINTER_MAX, quarter)),       // right
        ((260.0, 0.0), (quarter, -POINTER_MAX)),        // above
        ((260.0, 700.0), (quarter, POINTER_MAX)),       // below
        ((-50.0, -50.0), (-POINTER_MAX, -POINTER_MAX)), // top-left corner
        ((2000.0, 900.0), (POINTER_MAX, POINTER_MAX)),  // bottom-right corner
        ((740.0, 290.0), (POINTER_MAX, 0)),             // right edge itself
    ] {
        let state = aim(cursor, true);
        assert_eq!((state.x, state.y), want, "{cursor:?}");
        assert!(state.offscreen && !state.pressed, "{cursor:?}");
        assert_eq!(state.reply(0, 2), 0, "PRESSED");
    }
    assert_eq!(
        aim_to_pointer(None, Some((300.0, 200.0)), true, true),
        PointerState::default(),
        "no picture yet"
    );
    assert_eq!(
        aim_to_pointer(Some(RECT), None, false, true),
        PointerState::default(),
        "never inside"
    );
}

#[test]
fn cursor_left_keeps_the_last_position_offscreen() {
    let mut wii = mouse(WII);
    wii.moved(Some((260.0, 170.0)));
    wii.button(true, false);
    let size = (640, 528);
    let inside = wii.state(Some(RECT), size, false);
    assert!(!inside.offscreen && inside.pressed);
    wii.moved(None);
    let left = wii.state(Some(RECT), size, false);
    assert!(left.offscreen && !left.pressed, "button released on leave");
    assert_eq!((left.x, left.y), (inside.x, inside.y), "last position kept");
    assert!(!wii.over_picture(Some(RECT)));
    // Leaving past the right edge: the last event was already off the picture.
    wii.moved(Some((760.0, 170.0)));
    wii.moved(None);
    let parked = wii.state(Some(RECT), size, false);
    assert_eq!(parked.x, POINTER_MAX, "parked at the right edge");
    // The DS rule still forgets the cursor (byte-identical touch path).
    let mut ds = mouse(TouchScreenRule::DsBottomScreen);
    ds.moved(Some((260.0, 170.0)));
    ds.moved(None);
    assert_eq!(ds.cursor, None);
    assert_eq!(
        ds.state(Some(RECT), (256, 384), false),
        PointerState::default()
    );
}

#[test]
fn wide_resized_picture_still_maps_the_whole_frame() {
    // 640×528 frame shown 16:9 in a 1920×1080 window: pillarbox-free full width.
    let rect = compute_viewport((1920, 1080), (640, 528), 16.0 / 9.0, ScalingMode::AspectFit);
    let mut wii = mouse(WII);
    wii.moved(Some((
        f64::from(rect.x) + 0.75 * f64::from(rect.width),
        f64::from(rect.y) + 0.5 * f64::from(rect.height),
    )));
    let state = wii.state(Some(rect), (640, 528), false);
    assert_eq!((state.x, state.y), (fraction_to_pointer(0.75), 0));
    assert!(wii.over_picture(Some(rect)));
}

#[test]
fn os_cursor_hides_only_when_aiming_over_the_picture_with_the_menu_closed() {
    for aims in [false, true] {
        for over in [false, true] {
            for menu_open in [false, true] {
                let want = aims && over && !menu_open;
                assert_eq!(
                    hide_cursor(aims, over, menu_open),
                    want,
                    "{aims} {over} {menu_open}"
                );
            }
        }
    }
    let mut wii = mouse(WII);
    assert!(!wii.over_picture(Some(RECT)), "never inside");
    wii.moved(Some((420.0, 290.0)));
    assert!(wii.over_picture(Some(RECT)));
    assert!(!wii.over_picture(None), "no picture yet");
    wii.moved(Some((50.0, 290.0)));
    assert!(!wii.over_picture(Some(RECT)), "letterbox bar");
}
