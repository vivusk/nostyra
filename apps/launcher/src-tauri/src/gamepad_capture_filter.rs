//! Which pad event counts as "the input the user just pressed" during a capture,
//! kept free of a live SDL session so it is unit-testable with synthetic events.
//! Rules: buttons / triggers already held when a pad is seen are ignored until
//! released (no instant capture of a resting finger); a trigger counts once it
//! crosses half travel; Guide is reserved, so it never ends a capture.

use std::collections::HashSet;

use config_layers::RESERVED_GAMEPAD_INPUTS;
use sdl3::gamepad::{Axis, Button, GamepadType};
use serde::Serialize;

/// SDL button → our positional name (`config_layers::GAMEPAD_INPUT_NAMES`
/// minus the two triggers, which are axes in SDL).
pub const BUTTON_NAMES: [(Button, &str); 21] = [
    (Button::South, "south"),
    (Button::East, "east"),
    (Button::West, "west"),
    (Button::North, "north"),
    (Button::Back, "back"),
    (Button::Guide, "guide"),
    (Button::Start, "start"),
    (Button::LeftStick, "left_stick"),
    (Button::RightStick, "right_stick"),
    (Button::LeftShoulder, "left_shoulder"),
    (Button::RightShoulder, "right_shoulder"),
    (Button::DPadUp, "dpad_up"),
    (Button::DPadDown, "dpad_down"),
    (Button::DPadLeft, "dpad_left"),
    (Button::DPadRight, "dpad_right"),
    (Button::Misc1, "misc1"),
    (Button::LeftPaddle1, "left_paddle1"),
    (Button::LeftPaddle2, "left_paddle2"),
    (Button::RightPaddle1, "right_paddle1"),
    (Button::RightPaddle2, "right_paddle2"),
    (Button::Touchpad, "touchpad"),
];

/// SDL trigger axis → name.
pub const TRIGGER_NAMES: [(Axis, &str); 2] = [
    (Axis::TriggerLeft, "left_trigger"),
    (Axis::TriggerRight, "right_trigger"),
];

/// Half of the SDL trigger range (0..=32767), same rule as the runner.
pub const TRIGGER_PRESS_THRESHOLD: i16 = i16::MAX / 2;

/// Which printed labels the UI shows: decided by the last captured pad.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelStyle {
    #[default]
    Xbox,
    Sony,
}

impl LabelStyle {
    /// Sony labels for PlayStation pads, Xbox labels for everything else.
    pub fn of(kind: GamepadType) -> Self {
        match kind {
            GamepadType::PS3 | GamepadType::PS4 | GamepadType::PS5 => Self::Sony,
            _ => Self::Xbox,
        }
    }
}

pub fn button_name(button: Button) -> Option<&'static str> {
    BUTTON_NAMES
        .iter()
        .find(|(b, _)| *b == button)
        .map(|(_, name)| *name)
}

pub fn trigger_name(axis: Axis) -> Option<&'static str> {
    TRIGGER_NAMES
        .iter()
        .find(|(a, _)| *a == axis)
        .map(|(_, name)| *name)
}

/// Capture state across events: inputs (per pad id) still held from before.
#[derive(Debug, Default)]
pub struct CaptureFilter {
    ignored: HashSet<(u32, &'static str)>,
}

impl CaptureFilter {
    /// Marks an input that was already down when `pad` was opened.
    pub fn ignore_held(&mut self, pad: u32, name: &'static str) {
        self.ignored.insert((pad, name));
    }

    /// A button press: its name, unless held from before, reserved or unknown.
    pub fn button_down(&mut self, pad: u32, button: Button) -> Option<&'static str> {
        let name = button_name(button)?;
        self.accept(pad, name)
    }

    /// A release ends the "held from before" state.
    pub fn button_up(&mut self, pad: u32, button: Button) {
        if let Some(name) = button_name(button) {
            self.ignored.remove(&(pad, name));
        }
    }

    /// Trigger motion: its name once past half travel (stick axes never count).
    pub fn axis(&mut self, pad: u32, axis: Axis, value: i16) -> Option<&'static str> {
        let name = trigger_name(axis)?;
        if value < TRIGGER_PRESS_THRESHOLD {
            self.ignored.remove(&(pad, name));
            return None;
        }
        self.accept(pad, name)
    }

    fn accept(&self, pad: u32, name: &'static str) -> Option<&'static str> {
        (!self.ignored.contains(&(pad, name)) && !RESERVED_GAMEPAD_INPUTS.contains(&name))
            .then_some(name)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use config_layers::GAMEPAD_INPUT_NAMES;

    use super::*;

    #[test]
    fn every_config_pad_name_has_an_sdl_source() {
        let names: HashSet<&str> = BUTTON_NAMES
            .iter()
            .map(|(_, n)| *n)
            .chain(TRIGGER_NAMES.iter().map(|(_, n)| *n))
            .collect();
        let config: HashSet<&str> = GAMEPAD_INPUT_NAMES.into_iter().collect();
        assert_eq!(names, config);
        assert_eq!(names.len(), BUTTON_NAMES.len() + TRIGGER_NAMES.len());
        assert_eq!(button_name(Button::Misc2), None);
        assert_eq!(trigger_name(Axis::LeftX), None);
    }

    #[test]
    fn press_returns_the_name_and_guide_is_ignored() {
        let mut filter = CaptureFilter::default();
        assert_eq!(filter.button_down(1, Button::Guide), None);
        assert_eq!(filter.button_down(1, Button::South), Some("south"));
        assert_eq!(filter.button_down(2, Button::Misc2), None);
    }

    #[test]
    fn inputs_held_at_start_count_only_after_release() {
        let mut filter = CaptureFilter::default();
        filter.ignore_held(1, "east");
        filter.ignore_held(1, "right_trigger");
        assert_eq!(filter.button_down(1, Button::East), None);
        assert_eq!(
            filter.button_down(2, Button::East),
            Some("east"),
            "other pad"
        );
        filter.button_up(1, Button::East);
        assert_eq!(filter.button_down(1, Button::East), Some("east"));

        assert_eq!(filter.axis(1, Axis::TriggerRight, 32_000), None);
        assert_eq!(filter.axis(1, Axis::TriggerRight, 1_000), None);
        assert_eq!(
            filter.axis(1, Axis::TriggerRight, 20_000),
            Some("right_trigger")
        );
    }

    #[test]
    fn trigger_counts_past_half_travel_and_sticks_never() {
        let mut filter = CaptureFilter::default();
        assert_eq!(filter.axis(1, Axis::TriggerLeft, 10_000), None);
        assert_eq!(
            filter.axis(1, Axis::TriggerLeft, TRIGGER_PRESS_THRESHOLD),
            Some("left_trigger")
        );
        assert_eq!(filter.axis(1, Axis::LeftX, i16::MAX), None);
    }

    #[test]
    fn label_style_follows_the_pad_type() {
        assert_eq!(LabelStyle::of(GamepadType::PS5), LabelStyle::Sony);
        assert_eq!(LabelStyle::of(GamepadType::PS4), LabelStyle::Sony);
        assert_eq!(LabelStyle::of(GamepadType::XboxOne), LabelStyle::Xbox);
        assert_eq!(LabelStyle::of(GamepadType::Unknown), LabelStyle::Xbox);
        assert_eq!(
            serde_json::to_string(&LabelStyle::Sony).unwrap(),
            "\"sony\""
        );
    }
}
