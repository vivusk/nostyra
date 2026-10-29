//! Aiming pointer rules (`TouchScreenRule::aims`, Wii Remote IR aimed with the
//! mouse): the pointer state off the picture and the OS cursor visibility. Touch
//! rules (DS) never reach this module.

use input_mapping::{fraction_to_pointer, PointerState};
use video_renderer::ViewportRect;

use super::{on_picture, picture_fraction};

/// Aiming rule: cursor in the window and over the picture → pointer over the whole
/// frame, pressed as given. Otherwise offscreen, not pressed, x / y = the cursor
/// clamped to the picture edge (Dolphin ignores `IS_OFFSCREEN`, so the IR parks at
/// the edge instead of jumping to the centre). No cursor yet / no picture →
/// default (offscreen at the centre).
pub(crate) fn aim_to_pointer(
    viewport: Option<ViewportRect>,
    cursor: Option<(f64, f64)>,
    in_window: bool,
    pressed: bool,
) -> PointerState {
    let Some((fx, fy)) = picture_fraction(viewport, cursor) else {
        return PointerState::default();
    };
    let over = in_window && on_picture(fx) && on_picture(fy);
    PointerState {
        x: fraction_to_pointer(fx.clamp(0.0, 1.0)),
        y: fraction_to_pointer(fy.clamp(0.0, 1.0)),
        pressed: pressed && over,
        offscreen: !over,
    }
}

/// The OS cursor hides only for an aiming rule, over the picture, menu closed
/// (games draw their own pointer, which drifts from the OS cursor).
pub(crate) fn hide_cursor(aims: bool, over_picture: bool, menu_open: bool) -> bool {
    aims && over_picture && !menu_open
}

#[cfg(test)]
#[path = "pointer_events_aim_tests.rs"]
mod tests;
