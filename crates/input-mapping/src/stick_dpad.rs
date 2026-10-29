//! Left stick → D-pad unless the system reads the left stick raw
//! (`analog_stick_mode`), not rebindable (pad stick axes are not bindable inputs),
//! so analog-only pads and stick users get SNES directions.

use crate::joypad_state_snapshot::JoypadButton;

/// Left stick deflection (fraction of full range) that counts as a D-pad press.
pub const STICK_DEADZONE: f32 = 0.5;

/// Maps a left-stick position (SDL i16 axes, +y = down) to D-pad buttons.
pub fn stick_to_dpad(x: i16, y: i16) -> u16 {
    let threshold = (STICK_DEADZONE * f32::from(i16::MAX)) as i16;
    let mut mask = 0;
    if x <= -threshold {
        mask |= JoypadButton::Left.mask();
    } else if x >= threshold {
        mask |= JoypadButton::Right.mask();
    }
    if y <= -threshold {
        mask |= JoypadButton::Up.mask();
    } else if y >= threshold {
        mask |= JoypadButton::Down.mask();
    }
    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stick_deadzone_and_diagonals() {
        assert_eq!(stick_to_dpad(0, 0), 0);
        assert_eq!(stick_to_dpad(10_000, -10_000), 0); // inside 50% deadzone
        assert_eq!(stick_to_dpad(i16::MIN, 0), JoypadButton::Left.mask());
        assert_eq!(
            stick_to_dpad(i16::MAX, i16::MAX),
            JoypadButton::Right.mask() | JoypadButton::Down.mask()
        );
    }
}
