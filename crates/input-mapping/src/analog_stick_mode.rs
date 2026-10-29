//! What each pad stick does for the running system (`SystemSpec.analog_sticks`):
//! a stick reported raw (`RETRO_DEVICE_ANALOG`) never acts as the D-pad; the left
//! stick is the D-pad whenever it is not raw. `RightOnly` (Nintendo DS) keeps the
//! left-stick D-pad and sends only the right stick (the core's touch cursor).

pub use system_catalog::AnalogSticks;

use crate::stick_dpad::stick_to_dpad;

/// D-pad buttons from the left stick `(x, y)`: none when the left stick is raw.
pub fn left_stick_dpad(mode: AnalogSticks, x: i16, y: i16) -> u16 {
    match mode {
        AnalogSticks::Both => 0,
        AnalogSticks::None | AnalogSticks::RightOnly => stick_to_dpad(x, y),
    }
}

/// Raw `[left x, left y, right x, right y]` for the core: `None` when no stick is
/// raw; a stick that is not raw stays centred.
pub fn raw_stick_axes(mode: AnalogSticks, axes: [i16; 4]) -> Option<[i16; 4]> {
    match mode {
        AnalogSticks::None => None,
        AnalogSticks::Both => Some(axes),
        AnalogSticks::RightOnly => Some([0, 0, axes[2], axes[3]]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::joypad_state_snapshot::JoypadButton;

    const FULL: i16 = i16::MAX;

    #[test]
    fn left_stick_is_the_dpad_unless_both_sticks_are_raw() {
        let right = JoypadButton::Right.mask();
        assert_eq!(left_stick_dpad(AnalogSticks::None, FULL, 0), right);
        assert_eq!(left_stick_dpad(AnalogSticks::RightOnly, FULL, 0), right);
        assert_eq!(left_stick_dpad(AnalogSticks::Both, FULL, 0), 0);
    }

    #[test]
    fn right_only_sends_the_right_stick_and_centres_the_left() {
        let axes = [FULL, -FULL, 1_234, -5_678];
        assert_eq!(raw_stick_axes(AnalogSticks::None, axes), None);
        assert_eq!(raw_stick_axes(AnalogSticks::Both, axes), Some(axes));
        assert_eq!(
            raw_stick_axes(AnalogSticks::RightOnly, axes),
            Some([0, 0, 1_234, -5_678])
        );
    }
}
