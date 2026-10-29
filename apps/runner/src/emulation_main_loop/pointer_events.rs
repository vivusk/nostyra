//! Mouse → touch-screen / aiming pointer for systems with `SystemSpec.touch_screen`.
//! Window events only record the cursor (physical px, like the presenter's
//! viewport) and the left button; once per redraw the pointer state is rebuilt
//! from the current picture rect and core geometry (window resizes and layout swaps
//! apply at once). A press counts only inside the system's touch rect: the core
//! clamps a press elsewhere to the touch screen's edge. No press while the pause
//! menu is open; the button is released on focus loss, cursor leave and menu open.
//!
//! Aiming rules (`TouchScreenRule::aims`, Wii Remote IR): off the picture or out of
//! the window the pointer is offscreen but keeps its last position clamped to the
//! picture edge (Dolphin reads X/Y only, so the IR parks at the edge instead of
//! jumping to the centre), and the OS cursor is hidden while over the picture (the
//! game draws its own) unless the pause menu is open.

use input_mapping::{fraction_to_pointer, JoypadInputSource, PointerState};
use system_catalog::{touch_rect, TouchRect, TouchScreenRule};
use video_renderer::ViewportRect;

use self::aim::{aim_to_pointer, hide_cursor};
use super::EmulationApp;

#[path = "pointer_aim.rs"]
mod aim;

/// Raw mouse state of a pointer session.
pub(crate) struct MousePointer {
    rule: TouchScreenRule,
    /// Cursor in physical window px; `None` = outside the window (touch rules) /
    /// never inside yet (aiming rules keep the last position after a leave).
    cursor: Option<(f64, f64)>,
    /// The cursor is inside the window.
    in_window: bool,
    left_down: bool,
    /// We hid the OS cursor (aiming rule over the picture).
    cursor_hidden: bool,
}

impl MousePointer {
    /// `Some` (and the input source answers pointer queries) only for systems with
    /// a touch screen or aiming pointer; others keep pointer queries at 0.
    pub(crate) fn attach(
        rule: Option<TouchScreenRule>,
        input: &mut JoypadInputSource,
    ) -> Option<Self> {
        let rule = rule?;
        input.set_pointer(PointerState::default());
        Some(Self {
            rule,
            cursor: None,
            in_window: false,
            left_down: false,
            cursor_hidden: false,
        })
    }

    /// `CursorMoved` (`Some`) / `CursorLeft` (`None`, also releases the button;
    /// an aiming rule keeps the last position for the edge-clamped offscreen state).
    fn moved(&mut self, cursor: Option<(f64, f64)>) {
        self.in_window = cursor.is_some();
        if cursor.is_some() || !self.rule.aims() {
            self.cursor = cursor;
        }
        self.left_down &= cursor.is_some();
    }

    /// Left button; presses are ignored while the pause menu is open.
    fn button(&mut self, pressed: bool, menu_open: bool) {
        self.left_down = pressed && !menu_open;
    }

    /// The state the core reads, for the current picture rect and base geometry.
    fn state(
        &self,
        viewport: Option<ViewportRect>,
        size: (u32, u32),
        menu_open: bool,
    ) -> PointerState {
        let pressed = self.left_down && !menu_open;
        if self.rule.aims() {
            return aim_to_pointer(viewport, self.cursor, self.in_window, pressed);
        }
        let touch = touch_rect(self.rule, size.0, size.1);
        cursor_to_pointer(viewport, self.cursor, pressed, touch)
    }

    /// The cursor is in the window and over the picture.
    fn over_picture(&self, viewport: Option<ViewportRect>) -> bool {
        self.in_window
            && picture_fraction(viewport, self.cursor)
                .is_some_and(|(fx, fy)| on_picture(fx) && on_picture(fy))
    }
}

impl EmulationApp<'_> {
    pub(super) fn pointer_moved(&mut self, cursor: Option<(f64, f64)>) {
        if let Some(pointer) = self.pointer.as_mut() {
            pointer.moved(cursor);
        }
    }

    pub(super) fn pointer_button(&mut self, pressed: bool) {
        let menu_open = self.menu.is_open();
        if let Some(pointer) = self.pointer.as_mut() {
            pointer.button(pressed, menu_open);
        }
    }

    /// Focus lost / pause menu opened: no stuck touch, OS cursor shown again (paused
    /// redraws skip `update_pointer`; the next game redraw decides again).
    pub(super) fn release_pointer(&mut self) {
        if let Some(pointer) = self.pointer.as_mut() {
            pointer.left_down = false;
        }
        self.set_os_cursor_hidden(false);
    }

    /// Once per redraw before the frames: rebuilds what the core reads and whether
    /// the OS cursor shows.
    pub(super) fn update_pointer(&mut self) {
        let Some(pointer) = self.pointer.as_ref() else {
            return;
        };
        let geometry = self.session.av_info().geometry;
        let viewport = self.presenter.as_ref().and_then(|p| p.game_viewport());
        let size = (geometry.base_width, geometry.base_height);
        let menu_open = self.menu.is_open();
        let state = pointer.state(viewport, size, menu_open);
        let over = pointer.over_picture(viewport);
        let hide = hide_cursor(pointer.rule.aims(), over, menu_open);
        self.input.set_pointer(state);
        self.set_os_cursor_hidden(hide);
    }

    /// Shows / hides the OS cursor over the window, only when that changes (never
    /// for touch rules: they never ask to hide).
    fn set_os_cursor_hidden(&mut self, hide: bool) {
        let (Some(pointer), Some(window)) = (self.pointer.as_mut(), self.window.as_ref()) else {
            return;
        };
        if pointer.cursor_hidden != hide {
            window.set_cursor_visible(!hide);
            pointer.cursor_hidden = hide;
        }
    }
}

/// Cursor (physical px) over the picture `viewport` → pointer state over the whole
/// frame. Outside the picture (or no picture yet) → offscreen, not pressed;
/// `pressed` counts only inside `touch`.
pub(crate) fn cursor_to_pointer(
    viewport: Option<ViewportRect>,
    cursor: Option<(f64, f64)>,
    pressed: bool,
    touch: TouchRect,
) -> PointerState {
    let Some((fx, fy)) = picture_fraction(viewport, cursor) else {
        return PointerState::default();
    };
    if !(on_picture(fx) && on_picture(fy)) {
        return PointerState::default();
    }
    PointerState {
        x: fraction_to_pointer(fx),
        y: fraction_to_pointer(fy),
        pressed: pressed && touch.contains(fx as f32, fy as f32),
        offscreen: false,
    }
}

/// Cursor (physical px) → unclamped fractions of the picture `viewport`; `None`
/// without a cursor or a non-empty picture.
fn picture_fraction(
    viewport: Option<ViewportRect>,
    cursor: Option<(f64, f64)>,
) -> Option<(f64, f64)> {
    let (Some(rect), Some((cx, cy))) = (viewport, cursor) else {
        return None;
    };
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }
    let fx = (cx - f64::from(rect.x)) / f64::from(rect.width);
    let fy = (cy - f64::from(rect.y)) / f64::from(rect.height);
    Some((fx, fy))
}

/// A picture fraction lies on the picture (right / bottom edge excluded).
fn on_picture(fraction: f64) -> bool {
    (0.0..1.0).contains(&fraction)
}

#[cfg(test)]
#[path = "pointer_events_tests.rs"]
mod tests;
