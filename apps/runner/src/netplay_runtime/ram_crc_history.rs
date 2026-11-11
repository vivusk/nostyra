//! `(frame, ram crc)` of recently saved frames, for `--netplay-frames-limit` (F34):
//! the crc of the state at the start of frame N, kept apart from the state ring
//! (whose cells recycle after ~10 frames). A re-save of a frame after a rollback
//! overwrites its entry; entries below the confirmed frame are dropped, except the
//! frame the test waits for.

use std::collections::VecDeque;

use ggrs::Frame;

/// Entries kept at most (well above the 8-frame prediction window).
const CAPACITY: usize = 64;

#[derive(Debug)]
pub(crate) struct RamCrcHistory {
    entries: VecDeque<(Frame, u32)>,
    keep: Option<Frame>,
}

impl Default for RamCrcHistory {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(CAPACITY + 1),
            keep: None,
        }
    }
}

impl RamCrcHistory {
    /// Never drop `frame` (the frames-limit target).
    pub(crate) fn keep(&mut self, frame: Frame) {
        self.keep = Some(frame);
    }

    /// The crc of the state at the start of `frame` (overwrites a re-save).
    pub(crate) fn record(&mut self, frame: Frame, crc: u32) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.0 == frame) {
            entry.1 = crc;
            return;
        }
        self.entries.push_back((frame, crc));
        if self.entries.len() > CAPACITY {
            let keep = self.keep;
            if let Some(at) = self.entries.iter().position(|e| Some(e.0) != keep) {
                self.entries.remove(at);
            }
        }
    }

    /// Drops entries below `confirmed` (they can no longer change).
    pub(crate) fn prune(&mut self, confirmed: Frame) {
        let keep = self.keep;
        self.entries
            .retain(|&(frame, _)| frame >= confirmed || Some(frame) == keep);
    }

    pub(crate) fn get(&self, frame: Frame) -> Option<u32> {
        self.entries.iter().find(|e| e.0 == frame).map(|e| e.1)
    }

    /// Resync: the frame numbers start over.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

/// Per-frame crcs of the frames-limit test: RAM at the start of a frame, and a
/// running digest of every input applied before it (equal on every peer only if
/// each peer's input reached the others, whatever the test ROM keeps in RAM).
#[derive(Debug, Default)]
pub(crate) struct FrameCrcs {
    pub ram: RamCrcHistory,
    pub inputs: RamCrcHistory,
}

impl FrameCrcs {
    pub(crate) fn keep(&mut self, frame: Frame) {
        self.ram.keep(frame);
        self.inputs.keep(frame);
    }

    pub(crate) fn prune(&mut self, confirmed: Frame) {
        self.ram.prune(confirmed);
        self.inputs.prune(confirmed);
    }

    pub(crate) fn clear(&mut self) {
        self.ram.clear();
        self.inputs.clear();
    }

    /// Records the digest at the start of `frame + 1` from the one at `frame`.
    pub(crate) fn record_inputs(&mut self, frame: Frame, bytes: &[u8]) {
        let previous = self.inputs.get(frame).unwrap_or(0);
        let mut hasher = crc32fast::Hasher::new_with_initial(previous);
        hasher.update(bytes);
        self.inputs.record(frame + 1, hasher.finalize());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overwrite_prune_and_keep() {
        let mut history = RamCrcHistory::default();
        history.keep(5);
        for frame in 0..10 {
            history.record(frame, frame as u32);
        }
        history.record(5, 55); // re-save after a rollback
        assert_eq!(history.get(5), Some(55));
        history.prune(8);
        assert_eq!(history.get(4), None, "below confirmed: dropped");
        assert_eq!(history.get(5), Some(55), "the kept frame survives");
        assert_eq!(history.get(9), Some(9));
        for frame in 10..200 {
            history.record(frame, 1);
        }
        assert!(history.len() <= CAPACITY);
        assert_eq!(
            history.get(5),
            Some(55),
            "capacity never evicts the kept frame"
        );
    }
}
