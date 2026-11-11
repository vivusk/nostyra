//! The libretro `input_state` answer from a [`JoypadStateSnapshot`], shared by the
//! local [`crate::JoypadInputSource`] and the netplay [`crate::NetplayInputSource`]
//! so both reply the same way.

use libretro_host::{RETRO_DEVICE_ANALOG, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_POINTER};

use crate::joypad_state_snapshot::JoypadStateSnapshot;
use crate::pointer_state::PointerState;

/// Joypad buttons; analog sticks / triggers; the port-0 pointer when `pointer` is
/// set (touch-screen systems); every other device (mouse, keyboard, lightgun) and
/// out-of-range ports → 0.
pub(crate) fn reply(
    snapshot: &JoypadStateSnapshot,
    pointer: Option<&PointerState>,
    port: u32,
    device: u32,
    index: u32,
    id: u32,
) -> i16 {
    match (device, pointer) {
        (RETRO_DEVICE_JOYPAD, _) => snapshot.state(port as usize, id),
        (RETRO_DEVICE_ANALOG, _) => snapshot.analog_state(port as usize, index, id),
        (RETRO_DEVICE_POINTER, Some(pointer)) if port == 0 => pointer.reply(index, id),
        _ => 0,
    }
}

#[cfg(test)]
#[path = "snapshot_reply_tests.rs"]
mod tests;
