//! Analog trigger tests: pad pressure from the bound inputs, the index-2 reply, and
//! systems without analog triggers unchanged.

use config_layers::InputConfig;
use sdl3::gamepad::Button;

use super::*;
use crate::joypad_state_snapshot::JoypadStateSnapshot;

/// The Dreamcast pad's actions: both triggers are game buttons there, so the
/// default rewind / fast-forward trigger hotkeys yield on the pad.
const DC_ACTIONS: &[&str] = &[
    "b",
    "a",
    "y",
    "x",
    "start",
    "up",
    "down",
    "left",
    "right",
    "l2",
    "r2",
    "lstick_up",
    "lstick_down",
    "lstick_left",
    "lstick_right",
];

fn bindings(config: &InputConfig) -> InputBindings {
    InputBindings::from_config(config, Some(DC_ACTIONS)).0
}

#[test]
fn pad_triggers_give_their_travel_and_buttons_full_travel() {
    let defaults = bindings(&InputConfig::default());
    let half = |input| match input {
        GamepadInput::LeftTrigger => 0x3FFF,
        GamepadInput::RightTrigger => 9,
        GamepadInput::Button(_) => 0,
    };
    assert_eq!(pad_trigger_values(&defaults, half), [0x3FFF, 9]);
    let mut config = InputConfig::default();
    config
        .gamepad
        .insert("l2".into(), vec!["left_shoulder".into()]);
    let shoulder = bindings(&config);
    let pressed = |input| match input {
        GamepadInput::Button(Button::LeftShoulder) => TRIGGER_MAX,
        _ => 0,
    };
    assert_eq!(pad_trigger_values(&shoulder, pressed), [TRIGGER_MAX, 0]);
    assert_eq!(pad_trigger_values(&shoulder, |_| 0), [0, 0]);
}

#[test]
fn reply_uses_pressure_and_falls_back_to_the_digital_button() {
    let l2 = JoypadButton::L2.mask();
    let r2 = JoypadButton::R2.mask();
    assert_eq!(trigger_value([0x3FFF, 0], 0, 12), 0x3FFF, "half press");
    assert_eq!(trigger_value([0, 0], r2, 13), TRIGGER_MAX, "key held");
    assert_eq!(trigger_value([20_000, 0], l2, 12), 20_000, "pad past half");
    assert_eq!(trigger_value([0, 0], 0, 12), 0);
    assert_eq!(trigger_value([5, 5], l2 | r2, 0), 0, "other ids");
}

#[test]
fn snapshot_answers_index_2_only_for_analog_trigger_systems() {
    let mut snap = JoypadStateSnapshot::default();
    snap.press(0, JoypadButton::L2);
    snap.raise_triggers(0, [0, 1_000]);
    assert_eq!(
        snap.analog_state(0, ANALOG_BUTTON_INDEX, 12),
        0,
        "PS1 / N64: unchanged"
    );
    snap.enable_analog_triggers();
    assert_eq!(
        snap.analog_state(0, ANALOG_BUTTON_INDEX, 12),
        TRIGGER_MAX,
        "key"
    );
    assert_eq!(snap.analog_state(0, ANALOG_BUTTON_INDEX, 13), 1_000);
    snap.raise_triggers(0, [0, 500]);
    assert_eq!(
        snap.analog_state(0, ANALOG_BUTTON_INDEX, 13),
        1_000,
        "max of pads"
    );
    assert_eq!(
        snap.analog_state(1, ANALOG_BUTTON_INDEX, 13),
        0,
        "other port"
    );
    assert_eq!(snap.analog_state(9, ANALOG_BUTTON_INDEX, 13), 0);
    assert_eq!(snap.state(0, 12), 1, "the digital bit stays");
}

#[test]
fn suppressed_triggers_read_zero_until_released() {
    let mut values = [100, 200];
    hide_suppressed(&mut values, JoypadButton::R2.mask());
    assert_eq!(values, [100, 0], "only the suppressed trigger is hidden");
    hide_suppressed(&mut values, 0);
    assert_eq!(values, [100, 0]);
}
