//! One player's input for one frame, as it travels between netplay peers (ggrs
//! `Config::Input`). Fixed size, `Copy`, no allocation: RetroPad buttons as the
//! `RETRO_DEVICE_ID_JOYPAD_*` bitmask and the two sticks' raw axes. Analog axes are
//! sent only for systems with analog sticks (zero otherwise); trigger pressure and
//! held stick directions are folded in by the sender (`input-mapping`), so the
//! receiver needs nothing else to answer the core.

#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Version of the [`NetplayInput`] layout; peers with another version are refused in
/// the handshake (bump on any field change).
pub const WIRE_VERSION: u16 = 1;

/// Number of player slots a netplay frame carries (libretro ports 0–3).
pub const NETPLAY_PORTS: usize = 4;

/// One player's input for one frame.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NetplayInput {
    /// Bit `n` = `RETRO_DEVICE_ID_JOYPAD` id `n` pressed.
    pub buttons: u16,
    /// `[left x, left y, right x, right y]` in libretro range (±0x7FFF).
    pub axes: [i16; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(input: NetplayInput) -> Vec<u8> {
        bincode::serde::encode_to_vec(input, bincode::config::standard()).unwrap()
    }

    #[test]
    fn idle_and_full_inputs_stay_small_and_round_trip() {
        let idle = NetplayInput::default();
        assert_eq!(encode(idle).len(), 5, "1 B mask + 4 × 1 B axes (varint)");
        let full = NetplayInput {
            buttons: u16::MAX,
            axes: [i16::MAX, -i16::MAX, i16::MIN, 1],
        };
        let bytes = encode(full);
        assert!(bytes.len() <= 16, "{} bytes", bytes.len());
        let (back, read): (NetplayInput, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!((back, read), (full, bytes.len()));
    }

    #[test]
    fn layout_is_fixed() {
        assert_eq!(std::mem::size_of::<NetplayInput>(), 10);
        assert_eq!(WIRE_VERSION, 1);
    }
}
