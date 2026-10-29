//! One input's game actions as a single `u32`, so a key or pad input bound to a
//! button and a stick direction at once needs one lookup: RetroPad buttons
//! (`JoypadButton` masks) in the low 16 bits, stick directions (`StickDirection`
//! masks) from bit 16.

use crate::analog_stick_input::StickDirection;
use crate::joypad_state_snapshot::JoypadButton;

const STICK_SHIFT: u32 = 16;

/// Action name (`config_layers::PAD_ACTIONS`) → its bits.
pub(crate) fn action_bits(name: &str) -> Option<u32> {
    JoypadButton::from_action(name)
        .map(|button| u32::from(button.mask()))
        .or_else(|| StickDirection::from_action(name).map(|d| u32::from(d.mask()) << STICK_SHIFT))
}

/// The RetroPad button mask of `bits`.
pub(crate) fn buttons(bits: u32) -> u16 {
    (bits & 0xFFFF) as u16
}

/// The stick-direction mask of `bits`.
pub(crate) fn sticks(bits: u32) -> u8 {
    (bits >> STICK_SHIFT) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_converts_and_splits_back() {
        for name in config_layers::PAD_ACTIONS {
            let bits = action_bits(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(bits.count_ones(), 1, "{name}");
            let button = JoypadButton::from_action(name).map_or(0, JoypadButton::mask);
            let stick = StickDirection::from_action(name).map_or(0, StickDirection::mask);
            assert_eq!((buttons(bits), sticks(bits)), (button, stick), "{name}");
        }
        assert_eq!(action_bits("turbo"), None);
    }

    #[test]
    fn stick_actions_bind_to_keys_and_pad_buttons_without_warnings() {
        use config_layers::InputConfig;
        use sdl3::gamepad::Button;
        use winit::keyboard::KeyCode;

        use crate::gamepad_input_names::GamepadInput;
        use crate::input_bindings::InputBindings;

        let mut config = InputConfig::default();
        config
            .gamepad
            .insert("rstick_up".into(), vec!["north".into()]);
        config
            .keyboard
            .insert("lstick_left".into(), vec!["KeyX".into()]); // also A
        let (b, warnings) = InputBindings::from_config(&config, None);
        assert!(warnings.is_empty(), "{warnings:?}");
        let north = GamepadInput::Button(Button::North);
        let up = StickDirection::RightUp.mask();
        assert_eq!(b.pad_sticks(|input| input == north), up);
        assert_eq!(b.pad_mask(|input| input == north), JoypadButton::X.mask());
        assert_eq!(b.key_sticks(KeyCode::KeyX), StickDirection::LeftLeft.mask());
        assert_eq!(b.key_mask(KeyCode::KeyX), JoypadButton::A.mask());
        assert_eq!(b.key_sticks(KeyCode::KeyI), StickDirection::LeftUp.mask());
        assert_eq!(b.key_mask(KeyCode::KeyI), 0);
    }
}
