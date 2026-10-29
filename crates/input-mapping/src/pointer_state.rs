//! `RETRO_DEVICE_POINTER` state for systems with a touch screen
//! (`SystemSpec.touch_screen`): one pointer (index 0) on port 0, set by the runner
//! from the mouse over the displayed picture.

use libretro_host::{
    RETRO_DEVICE_ID_POINTER_COUNT, RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN,
    RETRO_DEVICE_ID_POINTER_PRESSED, RETRO_DEVICE_ID_POINTER_X, RETRO_DEVICE_ID_POINTER_Y,
};

/// Largest pointer coordinate; the frame spans `-POINTER_MAX..=POINTER_MAX`.
pub const POINTER_MAX: i16 = 0x7FFF;

/// Where the pointer is, in libretro pointer coordinates over the whole frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointerState {
    pub x: i16,
    pub y: i16,
    /// Pressed on the touch screen (never while `offscreen`).
    pub pressed: bool,
    /// The cursor is not over the picture.
    pub offscreen: bool,
}

impl Default for PointerState {
    /// No cursor over the picture, nothing pressed.
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            pressed: false,
            offscreen: true,
        }
    }
}

impl PointerState {
    /// Answer to `input_state(0, POINTER, index, id)`; other indices → 0.
    /// COUNT = 1 while the cursor is over the picture.
    pub fn reply(&self, index: u32, id: u32) -> i16 {
        if index != 0 {
            return 0;
        }
        match id {
            RETRO_DEVICE_ID_POINTER_X => self.x,
            RETRO_DEVICE_ID_POINTER_Y => self.y,
            RETRO_DEVICE_ID_POINTER_PRESSED => i16::from(self.pressed && !self.offscreen),
            RETRO_DEVICE_ID_POINTER_COUNT => i16::from(!self.offscreen),
            RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN => i16::from(self.offscreen),
            _ => 0,
        }
    }
}

/// A frame fraction (0 = left / top edge, 1 = right / bottom edge) → pointer
/// coordinate, clamped to `-POINTER_MAX..=POINTER_MAX`.
pub fn fraction_to_pointer(fraction: f64) -> i16 {
    let value = (fraction * 2.0 - 1.0) * f64::from(POINTER_MAX);
    value
        .round()
        .clamp(-f64::from(POINTER_MAX), f64::from(POINTER_MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_answers_index_zero_only() {
        let state = PointerState {
            x: -100,
            y: 200,
            pressed: true,
            offscreen: false,
        };
        assert_eq!(state.reply(0, RETRO_DEVICE_ID_POINTER_X), -100);
        assert_eq!(state.reply(0, RETRO_DEVICE_ID_POINTER_Y), 200);
        assert_eq!(state.reply(0, RETRO_DEVICE_ID_POINTER_PRESSED), 1);
        assert_eq!(state.reply(0, RETRO_DEVICE_ID_POINTER_COUNT), 1);
        assert_eq!(state.reply(0, RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN), 0);
        assert_eq!(state.reply(1, RETRO_DEVICE_ID_POINTER_X), 0, "second touch");
        assert_eq!(state.reply(0, 9), 0, "unknown id");
    }

    #[test]
    fn offscreen_pointer_is_never_pressed() {
        let state = PointerState {
            pressed: true,
            ..PointerState::default()
        };
        assert_eq!(state.reply(0, RETRO_DEVICE_ID_POINTER_PRESSED), 0);
        assert_eq!(state.reply(0, RETRO_DEVICE_ID_POINTER_COUNT), 0);
        assert_eq!(state.reply(0, RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN), 1);
    }

    #[test]
    fn fractions_map_onto_the_pointer_range() {
        assert_eq!(fraction_to_pointer(0.0), -POINTER_MAX);
        assert_eq!(fraction_to_pointer(0.5), 0);
        assert_eq!(fraction_to_pointer(1.0), POINTER_MAX);
        assert_eq!(fraction_to_pointer(0.75), 16_384);
        assert_eq!(fraction_to_pointer(-3.0), -POINTER_MAX, "clamped");
        assert_eq!(fraction_to_pointer(7.0), POINTER_MAX, "clamped");
    }
}
