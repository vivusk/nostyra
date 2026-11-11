//! `InputSource` answering the core from the inputs ggrs hands to one
//! `AdvanceFrame` (player n = libretro port n); records which ports the core asks
//! about (multitap probe evidence).

use libretro_host::{InputSource, RETRO_DEVICE_ANALOG, RETRO_DEVICE_JOYPAD};

use super::spike_config::NetplayInput;

/// Ports the spike can feed (plan: `PORT_COUNT` 2 → 4); the probe records up to 8.
pub(crate) const SPIKE_PORTS: usize = 4;

/// `RETRO_DEVICE_ID_JOYPAD_MASK`: the whole button mask in one query.
const JOYPAD_MASK_ID: u32 = 256;

/// The current frame's inputs and the set of queried ports.
#[derive(Debug, Default)]
pub(crate) struct FrameInputsSource {
    pub ports: [NetplayInput; SPIKE_PORTS],
    /// Bit n set: the core queried port n (any device) since the last reset.
    pub queried_ports: u8,
}

impl FrameInputsSource {
    /// Copies the first `SPIKE_PORTS` players' inputs (missing players: nothing held).
    pub(crate) fn set_inputs(&mut self, inputs: impl IntoIterator<Item = NetplayInput>) {
        self.ports = [NetplayInput::default(); SPIKE_PORTS];
        for (slot, input) in self.ports.iter_mut().zip(inputs) {
            *slot = input;
        }
    }
}

impl InputSource for FrameInputsSource {
    fn poll(&mut self) {}

    fn state(&mut self, port: u32, device: u32, index: u32, id: u32) -> i16 {
        if port < 8 {
            self.queried_ports |= 1 << port;
        }
        let Some(input) = self.ports.get(port as usize) else {
            return 0;
        };
        // Subclassed devices (multitap pads) carry the base type in the low byte.
        match (device & 0xff, index, id) {
            (RETRO_DEVICE_JOYPAD, _, JOYPAD_MASK_ID) => input.buttons as i16,
            (RETRO_DEVICE_JOYPAD, _, id) if id < 16 => i16::from(input.buttons >> id & 1 == 1),
            (RETRO_DEVICE_ANALOG, stick @ 0..=1, axis @ 0..=1) => {
                input.axes[(stick * 2 + axis) as usize]
            }
            _ => 0,
        }
    }
}
