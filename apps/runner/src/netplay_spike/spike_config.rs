//! ggrs config of the spike: the wire input (`NetplayInput`, plan Decisions "Input
//! wire") and the scripted per-player input every spike run feeds.

use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

/// One player's input for one frame: RetroPad bits (bit = `RETRO_DEVICE_ID_JOYPAD_*`)
/// and the two analog sticks (left x/y, right x/y).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub(crate) struct NetplayInput {
    pub buttons: u16,
    pub axes: [i16; 4],
}

/// Saved state of one frame: the core state blob and its desync checksum.
pub(crate) type SpikeState = (Vec<u8>, u128);

/// ggrs config generic over the address (matchbox `PeerId` online, `u8` in the
/// in-memory socket tests).
pub(crate) struct SpikeConfig<A>(PhantomData<A>);

impl<A: Clone + PartialEq + Eq + Hash + Debug + 'static> ggrs::Config for SpikeConfig<A> {
    type Input = NetplayInput;
    type InputPredictor = ggrs::PredictRepeatLast;
    type State = SpikeState;
    type Address = A;
}

/// Scripted input of `player` at `frame`: one of the 12 RetroPad buttons held for 7
/// frames, a different phase per player (so a remote input visibly changes RAM),
/// Start / Select skipped (no pause / reset menus), plus a left-stick step every 7 frames.
pub(crate) fn scripted_input(frame: i32, player: usize) -> NetplayInput {
    const BUTTONS: [u16; 10] = [0, 1, 4, 5, 6, 7, 8, 9, 10, 11]; // B Y Up Down Left Right A X L R
    let step = (frame.max(0) as usize / 7 + player * 3) % (BUTTONS.len() + 1);
    let buttons = BUTTONS.get(step).map_or(0, |&id| 1 << id);
    let x = ((frame.max(0) / 7 % 8) as i16 - 4) * 4096;
    NetplayInput {
        buttons,
        axes: [x, 0, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn players_get_different_scripts() {
        assert_ne!(scripted_input(0, 0), scripted_input(0, 1));
        assert_eq!(scripted_input(3, 0), scripted_input(5, 0), "held 7 frames");
        let size = bincode::serde::encode_to_vec(scripted_input(9, 1), bincode::config::standard())
            .expect("encode");
        assert!(size.len() <= 16, "varint wire size {}", size.len());
    }
}
