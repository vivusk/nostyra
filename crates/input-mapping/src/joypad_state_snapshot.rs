//! Per-frame joypad state for libretro ports 0–1: `RETRO_DEVICE_ID_JOYPAD_*`
//! bitmasks, plus raw stick axes, held stick directions and trigger pressure
//! (`RETRO_DEVICE_ANALOG`).

use crate::analog_stick_input::analog_value;
use crate::analog_trigger_input::{trigger_value, ANALOG_BUTTON_INDEX};
pub use crate::joypad_button::JoypadButton;

/// Number of player ports the runner exposes (M0: 2 players).
pub const PORT_COUNT: usize = 2;

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
mod tests {
    use super::*;

    #[test]
    fn press_and_query_by_retro_id() {
        let mut snap = JoypadStateSnapshot::default();
        snap.press(0, JoypadButton::A);
        snap.press(1, JoypadButton::Start);
        assert_eq!(snap.state(0, 8), 1);
        assert_eq!(snap.state(0, 0), 0);
        assert_eq!(snap.state(1, 3), 1);
        assert_eq!(snap.port_mask(0), 1 << 8);
    }

    #[test]
    fn analog_axes_and_holds_combine_per_port() {
        use crate::analog_stick_input::{StickDirection, ANALOG_MAX};
        let mut snap = JoypadStateSnapshot::default();
        assert_eq!(snap.analog_state(0, 0, 0), 0, "no stick → centred");
        snap.set_axes(0, [5_000, 0, 0, -7_000]);
        snap.hold_sticks(1, StickDirection::LeftRight.mask());
        assert_eq!(snap.analog_state(0, 0, 0), 5_000);
        assert_eq!(snap.analog_state(0, 1, 1), -7_000);
        assert_eq!(snap.analog_state(1, 0, 0), ANALOG_MAX);
        assert_eq!(snap.state(1, 7), 0, "a stick hold is not a D-pad press");
    }

    #[test]
    fn out_of_range_is_harmless() {
        let mut snap = JoypadStateSnapshot::default();
        snap.press(7, JoypadButton::B);
        assert_eq!(snap, JoypadStateSnapshot::default());
        assert_eq!(snap.state(5, 0), 0);
        assert_eq!(snap.state(0, 300), 0);
        assert_eq!(snap.analog_state(5, 0, 0), 0);
    }
}
