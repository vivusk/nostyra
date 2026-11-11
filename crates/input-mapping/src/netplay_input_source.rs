//! `InputSource` of a netplay session: the core reads a per-frame table of every
//! player's port, written by the rollback driver before each `retro_run`
//! (`AdvanceFrame { inputs }`), never local devices. Same reply as local play
//! (`snapshot_reply`), without pointer / mouse / keyboard devices.

use libretro_host::InputSource;
use netplay_wire::{NetplayInput, NETPLAY_PORTS};

use crate::joypad_state_snapshot::{JoypadStateSnapshot, PORT_COUNT};
use crate::netplay_input_wire::port_from_netplay_input;
use crate::snapshot_reply::reply;

const _: () = assert!(NETPLAY_PORTS == PORT_COUNT, "one wire slot per port");

/// The ports the core sees this frame.
#[derive(Debug, Default)]
pub struct NetplayInputSource {
    table: JoypadStateSnapshot,
}

impl NetplayInputSource {
    /// Every player's input for the next frame, slot = libretro port. `analog`: the
    /// system has analog sticks (else axes are ignored, whatever a peer sent).
    /// No allocation (called per advanced frame, re-simulations included).
    pub fn set_frame_inputs(&mut self, inputs: &[NetplayInput; NETPLAY_PORTS], analog: bool) {
        for (port, &input) in inputs.iter().enumerate() {
            let mut state = port_from_netplay_input(input);
            if !analog {
                state.axes = [0; 4];
            }
            self.table.set_port(port, state);
        }
    }

    /// The table the core reads (desync diagnostics, tests).
    pub fn table(&self) -> &JoypadStateSnapshot {
        &self.table
    }
}

impl InputSource for NetplayInputSource {
    /// Nothing to refresh: the driver set the table before `retro_run`.
    fn poll(&mut self) {}

    fn state(&mut self, port: u32, device: u32, index: u32, id: u32) -> i16 {
        reply(&self.table, None, port, device, index, id)
    }
}

#[cfg(test)]
mod tests {
    use libretro_host::{RETRO_DEVICE_ANALOG, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_POINTER};

    use super::*;

    const MOUSE: u32 = 2;
    const KEYBOARD: u32 = 3;
    const LIGHTGUN: u32 = 4;

    fn inputs() -> [NetplayInput; NETPLAY_PORTS] {
        let mut inputs = [NetplayInput::default(); NETPLAY_PORTS];
        inputs[0].buttons = 1 << 8; // A
        inputs[2] = NetplayInput {
            buttons: 1 << 3, // Start
            axes: [5_000, 0, 0, -6_000],
        };
        inputs
    }

    #[test]
    fn answers_joypad_and_analog_from_the_frame_table() {
        let mut source = NetplayInputSource::default();
        source.set_frame_inputs(&inputs(), true);
        source.poll();
        assert_eq!(source.state(0, RETRO_DEVICE_JOYPAD, 0, 8), 1);
        assert_eq!(source.state(2, RETRO_DEVICE_JOYPAD, 0, 3), 1, "port 2");
        assert_eq!(source.state(1, RETRO_DEVICE_JOYPAD, 0, 8), 0);
        assert_eq!(source.state(2, RETRO_DEVICE_ANALOG, 0, 0), 5_000);
        assert_eq!(source.state(2, RETRO_DEVICE_ANALOG, 1, 1), -6_000);
    }

    #[test]
    fn other_devices_and_ports_past_four_answer_zero() {
        let mut source = NetplayInputSource::default();
        let all = NetplayInput {
            buttons: u16::MAX,
            axes: [7; 4],
        };
        source.set_frame_inputs(&[all; NETPLAY_PORTS], true);
        for device in [RETRO_DEVICE_POINTER, MOUSE, KEYBOARD, LIGHTGUN] {
            assert_eq!(source.state(0, device, 0, 0), 0, "device {device}");
        }
        assert_eq!(source.state(0, RETRO_DEVICE_JOYPAD, 0, 0), 1);
        assert_eq!(source.state(4, RETRO_DEVICE_JOYPAD, 0, 0), 0);
        assert_eq!(source.state(u32::MAX, RETRO_DEVICE_ANALOG, 0, 0), 0);
    }

    #[test]
    fn digital_systems_ignore_received_axes_and_frames_replace_each_other() {
        let mut source = NetplayInputSource::default();
        source.set_frame_inputs(&inputs(), false);
        assert_eq!(source.state(2, RETRO_DEVICE_ANALOG, 0, 0), 0);
        source.set_frame_inputs(&[NetplayInput::default(); NETPLAY_PORTS], false);
        assert_eq!(source.table(), &JoypadStateSnapshot::default());
    }
}
