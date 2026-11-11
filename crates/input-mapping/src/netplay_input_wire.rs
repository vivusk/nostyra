//! One port of a [`crate::JoypadStateSnapshot`] ↔ the netplay wire struct
//! [`NetplayInput`] (`netplay-wire`). The wire has buttons and stick axes only, so
//! the sender folds the rest in: held stick directions become full deflection on
//! the axes (analog systems) or D-pad bits (digital systems), trigger pressure past
//! half travel becomes the L2 / R2 bits. The receiver gets what the core would read
//! from the sender's port.

use netplay_wire::NetplayInput;

use crate::analog_stick_input::{analog_value, StickDirection};
use crate::gamepad_input_names::trigger_held;
use crate::joypad_state_snapshot::{JoypadButton, PortState};

/// Left-stick hold → D-pad button, for systems without analog sticks.
const STICK_TO_DPAD: [(StickDirection, JoypadButton); 4] = [
    (StickDirection::LeftUp, JoypadButton::Up),
    (StickDirection::LeftDown, JoypadButton::Down),
    (StickDirection::LeftLeft, JoypadButton::Left),
    (StickDirection::LeftRight, JoypadButton::Right),
];

/// What this player sends for a frame. `carry_analog`: the system has analog
/// sticks (`SystemSpec.analog_sticks != None`); otherwise the axes stay zero.
pub fn netplay_input_from_port(state: PortState, carry_analog: bool) -> NetplayInput {
    let mut buttons = state.buttons;
    let triggers = [JoypadButton::L2, JoypadButton::R2];
    for (pressure, button) in state.triggers.into_iter().zip(triggers) {
        if trigger_held(pressure) {
            buttons |= button.mask();
        }
    }
    let mut axes = [0; 4];
    if carry_analog {
        for (slot, axis) in axes.iter_mut().enumerate() {
            let (stick, id) = ((slot / 2) as u32, (slot % 2) as u32);
            *axis = analog_value(state.axes, state.stick_holds, stick, id);
        }
    } else {
        for (direction, button) in STICK_TO_DPAD {
            if state.stick_holds & direction.mask() != 0 {
                buttons |= button.mask();
            }
        }
    }
    NetplayInput { buttons, axes }
}

/// The port a received input stands for (no holds or pressure left to resolve).
pub fn port_from_netplay_input(input: NetplayInput) -> PortState {
    PortState {
        buttons: input.buttons,
        axes: input.axes,
        ..PortState::default()
    }
}

#[cfg(test)]
#[path = "netplay_input_wire_tests.rs"]
mod tests;
