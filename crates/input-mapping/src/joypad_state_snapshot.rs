//! Per-frame joypad state for libretro ports 0–3: `RETRO_DEVICE_ID_JOYPAD_*`
//! bitmasks, plus raw stick axes, held stick directions and trigger pressure
//! (`RETRO_DEVICE_ANALOG`).

use crate::analog_stick_input::analog_value;
use crate::analog_trigger_input::{trigger_value, ANALOG_BUTTON_INDEX};
pub use crate::joypad_button::JoypadButton;

/// Number of player ports the runner exposes (M20: 4, for netplay multitap rooms;
/// local pads fill them in connection order, a 5th pad is ignored).
pub const PORT_COUNT: usize = 4;

/// Everything one port holds for one frame (netplay sends and receives a port at a
/// time).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PortState {
    /// `RETRO_DEVICE_ID_JOYPAD_*` bitmask.
    pub buttons: u16,
    /// Raw `[left x, left y, right x, right y]`.
    pub axes: [i16; 4],
    /// `[L2, R2]` pressure.
    pub triggers: [i16; 2],
    /// Held [`crate::analog_stick_input::StickDirection`] bits.
    pub stick_holds: u8,
}

/// Button bitmasks and analog sticks for all ports, rebuilt once per frame before
/// `retro_run`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JoypadStateSnapshot {
    ports: [u16; PORT_COUNT],
    /// Raw `[left x, left y, right x, right y]` (only for systems with analog sticks).
    axes: [[i16; 4]; PORT_COUNT],
    /// Held [`crate::analog_stick_input::StickDirection`] bits.
    stick_holds: [u8; PORT_COUNT],
    /// `[L2, R2]` pressure (only for systems with analog triggers).
    triggers: [[i16; 2]; PORT_COUNT],
    /// The system has analog triggers: index 2 of `RETRO_DEVICE_ANALOG` answers.
    analog_triggers: bool,
}

impl JoypadStateSnapshot {
    /// ORs `mask` into `port` (out-of-range ports are ignored).
    pub fn press_mask(&mut self, port: usize, mask: u16) {
        if let Some(bits) = self.ports.get_mut(port) {
            *bits |= mask;
        }
    }

    pub fn press(&mut self, port: usize, button: JoypadButton) {
        self.press_mask(port, button.mask());
    }

    /// libretro `input_state` answer for a joypad id: 1 if pressed, else 0.
    pub fn state(&self, port: usize, id: u32) -> i16 {
        let bits = self.ports.get(port).copied().unwrap_or(0);
        i16::from(id < 16 && bits & (1 << id) != 0)
    }

    pub fn port_mask(&self, port: usize) -> u16 {
        self.ports.get(port).copied().unwrap_or(0)
    }

    /// Raw stick axes of `port` (centred for out-of-range ports).
    pub fn axes(&self, port: usize) -> [i16; 4] {
        self.axes.get(port).copied().unwrap_or_default()
    }

    /// Held stick directions of `port`.
    pub fn stick_holds(&self, port: usize) -> u8 {
        self.stick_holds.get(port).copied().unwrap_or(0)
    }

    /// `[L2, R2]` pressure of `port`.
    pub fn triggers(&self, port: usize) -> [i16; 2] {
        self.triggers.get(port).copied().unwrap_or_default()
    }

    /// Whether index 2 of `RETRO_DEVICE_ANALOG` answers trigger pressure.
    pub fn analog_triggers(&self) -> bool {
        self.analog_triggers
    }

    /// Everything `port` holds (default for out-of-range ports).
    pub fn port(&self, port: usize) -> PortState {
        PortState {
            buttons: self.port_mask(port),
            axes: self.axes(port),
            triggers: self.triggers(port),
            stick_holds: self.stick_holds(port),
        }
    }

    /// Replaces `port` with `state` (out-of-range ports are ignored).
    pub fn set_port(&mut self, port: usize, state: PortState) {
        if port >= PORT_COUNT {
            return;
        }
        self.ports[port] = state.buttons;
        self.axes[port] = state.axes;
        self.triggers[port] = state.triggers;
        self.stick_holds[port] = state.stick_holds;
    }

    /// Raw stick axes of the pad on `port` (out-of-range ports are ignored).
    pub fn set_axes(&mut self, port: usize, axes: [i16; 4]) {
        if let Some(slot) = self.axes.get_mut(port) {
            *slot = axes;
        }
    }

    /// ORs held stick directions into `port`.
    pub fn hold_sticks(&mut self, port: usize, mask: u8) {
        if let Some(holds) = self.stick_holds.get_mut(port) {
            *holds |= mask;
        }
    }

    /// Index 2 of `RETRO_DEVICE_ANALOG` answers L2 / R2 pressure (the running
    /// system has analog triggers).
    pub fn enable_analog_triggers(&mut self) {
        self.analog_triggers = true;
    }

    /// Raises `port`'s `[L2, R2]` pressure to at least `values` (pads on one port).
    pub fn raise_triggers(&mut self, port: usize, values: [i16; 2]) {
        if let Some(slot) = self.triggers.get_mut(port) {
            *slot = [slot[0].max(values[0]), slot[1].max(values[1])];
        }
    }

    /// libretro `input_state` answer for `RETRO_DEVICE_ANALOG` (index = stick, id =
    /// axis; index 2 = analog buttons L2 / R2 when enabled, else 0).
    pub fn analog_state(&self, port: usize, index: u32, id: u32) -> i16 {
        if index == ANALOG_BUTTON_INDEX {
            return match self.triggers.get(port) {
                Some(&triggers) if self.analog_triggers => {
                    trigger_value(triggers, self.port_mask(port), id)
                }
                _ => 0,
            };
        }
        match (self.axes.get(port), self.stick_holds.get(port)) {
            (Some(axes), Some(holds)) => analog_value(*axes, *holds, index, id),
            _ => 0,
        }
    }
}

#[cfg(test)]
#[path = "joypad_state_snapshot_tests.rs"]
mod tests;
