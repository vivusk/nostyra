//! ggrs requests → [`RollbackHost`] calls, in order. Tracks the frame being advanced
//! and `head_frame` (highest frame ever advanced): `is_new = frame > head_frame`
//! (F20 forward path), and hands `last_due` to the tick's last advance only (F35).
//! State bytes live in a ring of `max_prediction_window + 2` reusable buffers indexed
//! by frame; the ggrs cell only stores the frame number, so steady state allocates
//! nothing (ggrs never loads a frame older than its prediction window).

use ggrs::{Frame, GgrsRequest, NULL_FRAME};

use super::RollbackHost;
use crate::rollback_config::NetplayConfig;
use crate::NetplayError;

/// Frame bookkeeping of one session.
#[derive(Debug, Clone, Copy)]
pub(super) struct FrameCursor {
    /// Frame the next `AdvanceFrame` advances.
    pub current: Frame,
    /// Highest frame ever advanced (`NULL_FRAME` before the first).
    pub head: Frame,
}

impl FrameCursor {
    pub const START: Self = Self {
        current: 0,
        head: NULL_FRAME,
    };
}

/// Reusable state buffers.
pub(super) struct StateRing {
    slots: Vec<(Frame, Vec<u8>)>,
    /// Saves that had to grow a buffer (first fill of each slot; then 0).
    pub grows: u64,
    pub saves: u64,
}

impl StateRing {
    pub fn new(len: usize) -> Self {
        Self {
            slots: (0..len.max(1)).map(|_| (NULL_FRAME, Vec::new())).collect(),
            grows: 0,
            saves: 0,
        }
    }

    /// Forgets every saved frame, keeps the buffers (rebuild).
    pub fn reset(&mut self, len: usize) {
        self.slots
            .resize_with(len.max(1), || (NULL_FRAME, Vec::new()));
        self.slots.iter_mut().for_each(|slot| slot.0 = NULL_FRAME);
        (self.grows, self.saves) = (0, 0);
    }

    fn index(&self, frame: Frame) -> usize {
        frame.rem_euclid(self.slots.len() as i32) as usize
    }

    fn save(&mut self, frame: Frame, host: &mut dyn RollbackHost) -> Result<u128, NetplayError> {
        let index = self.index(frame);
        let (saved, buf) = &mut self.slots[index];
        let capacity = buf.capacity();
        buf.clear();
        let checksum = host.save(frame, buf)?;
        *saved = frame;
        self.saves += 1;
        if buf.capacity() > capacity {
            self.grows += 1;
        }
        Ok(checksum)
    }

    fn load(&self, frame: Frame, host: &mut dyn RollbackHost) -> Result<(), NetplayError> {
        let (saved, buf) = &self.slots[self.index(frame)];
        if *saved != frame {
            return Err(NetplayError::Rollback(format!(
                "state of frame {frame} is gone"
            )));
        }
        host.load(frame, buf)
    }
}

/// Counts of one tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Applied {
    pub new_frames: u32,
    pub resim_frames: u32,
}

/// Fulfils `requests` in order.
pub(super) fn apply_requests(
    requests: Vec<GgrsRequest<NetplayConfig>>,
    cursor: &mut FrameCursor,
    ring: &mut StateRing,
    host: &mut dyn RollbackHost,
    last_due: bool,
) -> Result<Applied, NetplayError> {
    let last_advance = requests
        .iter()
        .rposition(|r| matches!(r, GgrsRequest::AdvanceFrame { .. }));
    let mut applied = Applied::default();
    for (index, request) in requests.into_iter().enumerate() {
        match request {
            GgrsRequest::SaveGameState { cell, frame } => {
                let checksum = ring.save(frame, host)?;
                cell.save(frame, Some(frame), Some(checksum));
            }
            GgrsRequest::LoadGameState { cell, frame } => {
                let saved = cell.data().map(|data| *data);
                if saved != Some(frame) {
                    return Err(NetplayError::Rollback(format!(
                        "cell does not hold frame {frame}"
                    )));
                }
                ring.load(frame, host)?;
                cursor.current = frame;
            }
            GgrsRequest::AdvanceFrame { inputs } => {
                let frame = cursor.current;
                let is_new = frame > cursor.head;
                let last_of_tick = last_due && Some(index) == last_advance;
                host.advance(frame, &inputs, is_new, last_of_tick);
                if is_new {
                    cursor.head = frame;
                    applied.new_frames += 1;
                } else {
                    applied.resim_frames += 1;
                }
                cursor.current = frame + 1;
            }
        }
    }
    Ok(applied)
}

#[cfg(test)]
#[path = "request_apply_tests.rs"]
mod tests;
