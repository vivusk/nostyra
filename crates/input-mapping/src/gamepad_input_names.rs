//! Positional gamepad input names (`config_layers::GAMEPAD_INPUT_NAMES`) ↔ SDL
//! buttons and the two analog triggers. Our own enum is the stable surface:
//! SDL's `Button` naming / variant set may shift between sdl3 versions, and
//! triggers are axes in SDL but plain bindable inputs for users. Labels live in
//! `config_layers::GAMEPAD_INPUT_LABELS` (the launcher must not need SDL names).

use sdl3::gamepad::{Axis, Button};

/// One bindable pad input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadInput {
    Button(Button),
    /// Counts as pressed past half travel ([`trigger_held`]).
    LeftTrigger,
    /// Counts as pressed past half travel ([`trigger_held`]).
    RightTrigger,
}

/// A trigger pulled past half way (SDL trigger axes range 0..=32767) counts as
/// pressed, whether it is bound to a game button or a hotkey.
const TRIGGER_HELD_THRESHOLD: i16 = i16::MAX / 2;

/// Whether a trigger axis value counts as pressed.
pub fn trigger_held(value: i16) -> bool {
    value >= TRIGGER_HELD_THRESHOLD
}

use GamepadInput::Button as B;

/// Config name → input; same order as `GAMEPAD_INPUT_NAMES`.
const GAMEPAD_INPUTS: [(&str, GamepadInput); 23] = [
    ("south", B(Button::South)),
    ("east", B(Button::East)),
    ("west", B(Button::West)),
    ("north", B(Button::North)),
    ("back", B(Button::Back)),
    ("guide", B(Button::Guide)),
    ("start", B(Button::Start)),
    ("left_stick", B(Button::LeftStick)),
    ("right_stick", B(Button::RightStick)),
    ("left_shoulder", B(Button::LeftShoulder)),
    ("right_shoulder", B(Button::RightShoulder)),
    ("dpad_up", B(Button::DPadUp)),
    ("dpad_down", B(Button::DPadDown)),
    ("dpad_left", B(Button::DPadLeft)),
    ("dpad_right", B(Button::DPadRight)),
    ("misc1", B(Button::Misc1)),
    ("left_paddle1", B(Button::LeftPaddle1)),
    ("left_paddle2", B(Button::LeftPaddle2)),
    ("right_paddle1", B(Button::RightPaddle1)),
    ("right_paddle2", B(Button::RightPaddle2)),
    ("touchpad", B(Button::Touchpad)),
    ("left_trigger", GamepadInput::LeftTrigger),
    ("right_trigger", GamepadInput::RightTrigger),
];

impl GamepadInput {
    /// The input a config name stands for (`None` = unknown name).
    pub fn from_name(name: &str) -> Option<Self> {
        GAMEPAD_INPUTS
            .iter()
            .find(|(input_name, _)| *input_name == name)
            .map(|(_, input)| *input)
    }

    /// Config name (`None` for SDL buttons we do not offer, e.g. `Misc2`).
    pub fn name(self) -> Option<&'static str> {
        GAMEPAD_INPUTS
            .iter()
            .find(|(_, input)| *input == self)
            .map(|(name, _)| *name)
    }

    /// The trigger input an SDL axis drives (`None` for stick axes).
    pub fn trigger(axis: Axis) -> Option<Self> {
        match axis {
            Axis::TriggerLeft => Some(Self::LeftTrigger),
            Axis::TriggerRight => Some(Self::RightTrigger),
            _ => None,
        }
    }

    /// Whether this input is down on a pad, given its button and axis readers
    /// (closures keep this testable without a real SDL pad).
    pub fn is_held(self, button: impl Fn(Button) -> bool, axis: impl Fn(Axis) -> i16) -> bool {
        match self {
            Self::Button(b) => button(b),
            Self::LeftTrigger => trigger_held(axis(Axis::TriggerLeft)),
            Self::RightTrigger => trigger_held(axis(Axis::TriggerRight)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use config_layers::GAMEPAD_INPUT_NAMES;

    use super::*;

    #[test]
    fn every_config_pad_name_converts_round_trips_and_is_unique() {
        assert_eq!(GAMEPAD_INPUTS.len(), GAMEPAD_INPUT_NAMES.len());
        for name in GAMEPAD_INPUT_NAMES {
            let input = GamepadInput::from_name(name);
            assert!(input.is_some(), "{name} has no pad input");
            assert_eq!(input.and_then(GamepadInput::name), Some(name));
        }
        let inputs: HashSet<_> = GAMEPAD_INPUTS.iter().map(|(_, input)| *input).collect();
        assert_eq!(inputs.len(), GAMEPAD_INPUTS.len());
        assert_eq!(
            GamepadInput::from_name("a"),
            None,
            "SDL mapping names unused"
        );
        assert_eq!(GamepadInput::Button(Button::Misc2).name(), None);
    }

    #[test]
    fn trigger_threshold_is_half_way() {
        assert!(!trigger_held(0));
        assert!(!trigger_held(10_000));
        assert!(trigger_held(20_000));
        assert!(trigger_held(i16::MAX));
    }

    #[test]
    fn held_reads_buttons_and_triggers() {
        let pressed = |b: Button| b == Button::RightStick;
        let axis = |a: Axis| {
            if a == Axis::TriggerRight {
                30_000
            } else {
                5_000
            }
        };
        assert!(GamepadInput::Button(Button::RightStick).is_held(pressed, axis));
        assert!(!GamepadInput::Button(Button::South).is_held(pressed, axis));
        assert!(GamepadInput::RightTrigger.is_held(pressed, axis));
        assert!(!GamepadInput::LeftTrigger.is_held(pressed, axis));
        assert_eq!(
            GamepadInput::trigger(Axis::TriggerLeft),
            Some(GamepadInput::LeftTrigger)
        );
        assert_eq!(GamepadInput::trigger(Axis::LeftX), None);
    }
}
