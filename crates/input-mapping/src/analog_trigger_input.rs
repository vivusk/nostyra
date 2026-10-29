//! Analog triggers for systems that have them (`SystemSpec.analog_triggers`,
//! Dreamcast): the `RETRO_DEVICE_ANALOG` reply for index 2
//! (`RETRO_DEVICE_INDEX_ANALOG_BUTTON`), ids L2 / R2, is the pressure of the inputs
//! bound to `l2` / `r2`. A pad trigger gives its travel (0..=0x7FFF); a button or key
//! bound to them gives full travel while held. The digital L2 / R2 bits stay as
//! they are (cores fall back to them). Other systems answer 0 on index 2, as before.

use sdl3::gamepad::{Axis, Gamepad};

use crate::analog_stick_mode::AnalogSticks;
use crate::gamepad_input_names::GamepadInput;
use crate::input_bindings::InputBindings;
use crate::joypad_state_snapshot::JoypadButton;

/// libretro analog button range is 0..=0x7FFF.
pub const TRIGGER_MAX: i16 = 0x7FFF;
/// `RETRO_DEVICE_INDEX_ANALOG_BUTTON`.
pub const ANALOG_BUTTON_INDEX: u32 = 2;
/// The two analog buttons, in reply order.
const TRIGGERS: [JoypadButton; 2] = [JoypadButton::L2, JoypadButton::R2];

/// Which analog inputs the running system's pad has (`SystemSpec.analog_sticks` /
/// `analog_triggers`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnalogInputs {
    pub sticks: AnalogSticks,
    pub triggers: bool,
}

/// Analog value of a pad input: trigger travel, or full / 0 for a button.
pub fn input_value(pad: &Gamepad, input: GamepadInput) -> i16 {
    match input {
        GamepadInput::LeftTrigger => pad.axis(Axis::TriggerLeft),
        GamepadInput::RightTrigger => pad.axis(Axis::TriggerRight),
        GamepadInput::Button(button) => i16::from(pad.button(button)) * TRIGGER_MAX,
    }
}

/// `[L2, R2]` pressure of `pad`, 0 for a trigger whose bit is in `suppressed` (held
/// when the pause menu closed: hidden from the core until released).
pub fn read_pad_triggers(bindings: &InputBindings, pad: &Gamepad, suppressed: u16) -> [i16; 2] {
    let mut values = pad_trigger_values(bindings, |input| input_value(pad, input));
    hide_suppressed(&mut values, suppressed);
    values
}

fn hide_suppressed(values: &mut [i16; 2], suppressed: u16) {
    for (value, trigger) in values.iter_mut().zip(TRIGGERS) {
        if suppressed & trigger.mask() != 0 {
            *value = 0;
        }
    }
}

/// `[L2, R2]` pressure on one pad: the largest value among the pad inputs bound to
/// each (`value` reads an input: trigger travel, or full / 0 for a button).
pub fn pad_trigger_values(
    bindings: &InputBindings,
    value: impl Fn(GamepadInput) -> i16,
) -> [i16; 2] {
    let mut out = [0i16; 2];
    for (input, bits) in bindings.pad_game_inputs() {
        for (slot, trigger) in TRIGGERS.iter().enumerate() {
            if bits & u32::from(trigger.mask()) != 0 {
                out[slot] = out[slot].max(value(input).clamp(0, TRIGGER_MAX));
            }
        }
    }
    out
}

/// Reply for index 2, `id` = `RETRO_DEVICE_ID_JOYPAD_L2` / `R2`: the pressure, or
/// full travel when only a digital source (a key) holds the button.
pub fn trigger_value(triggers: [i16; 2], digital: u16, id: u32) -> i16 {
    let Some(slot) = TRIGGERS.iter().position(|t| *t as u32 == id) else {
        return 0;
    };
    match triggers[slot] {
        0 if digital & TRIGGERS[slot].mask() != 0 => TRIGGER_MAX,
        value => value,
    }
}

#[cfg(test)]
#[path = "analog_trigger_input_tests.rs"]
mod tests;
