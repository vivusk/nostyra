//! `--netplay-frames-limit N` (loopback test, F24 / F34): local input stops at
//! frame N (neutral input after), frames keep ticking until N is confirmed and a
//! settle passed, then `ram_crc=<hex> input_crc=<hex> frames=N` is printed (crc of the
//! state at the start of confirmed frame N, from the crc history) and the runner
//! exits 0.

use ggrs::Frame;

use super::ram_crc_history::FrameCrcs;

/// Frames run after N is confirmed (drains events; nothing is read afterwards).
pub(crate) const SETTLE_FRAMES: Frame = 60;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FramesLimit {
    target: Frame,
    /// Frames to run after N is confirmed (spectators: 0, they confirm as they
    /// advance and the host closes its relay once its own settle ended).
    settle: Frame,
    /// Head frame when N became confirmed.
    settle_from: Option<Frame>,
    printed: bool,
}

impl FramesLimit {
    pub(crate) fn new(target: Frame, settle: Frame) -> Self {
        Self {
            target,
            settle,
            settle_from: None,
            printed: false,
        }
    }

    pub(crate) fn target(&self) -> Frame {
        self.target
    }

    /// Whether the local player still sends its input for `frame`.
    pub(crate) fn input_allowed(&self, frame: Frame) -> bool {
        frame < self.target
    }

    /// The line to print once N is confirmed and settled (once).
    pub(crate) fn check(
        &mut self,
        confirmed: Frame,
        head: Frame,
        history: &FrameCrcs,
    ) -> Option<String> {
        if self.printed || confirmed < self.target {
            return None;
        }
        let crc = history.ram.get(self.target)?;
        let inputs = history.inputs.get(self.target).unwrap_or(0);
        let from = *self.settle_from.get_or_insert(head);
        if head < from + self.settle {
            return None;
        }
        self.printed = true;
        let target = self.target;
        Some(format!(
            "ram_crc={crc:08x} input_crc={inputs:08x} frames={target}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_once_after_confirmation_and_settle() {
        let mut history = FrameCrcs::default();
        history.keep(10);
        history.ram.record(10, 0xabcd);
        history.inputs.record(10, 7);
        let mut limit = FramesLimit::new(10, SETTLE_FRAMES);
        assert!(limit.input_allowed(9) && !limit.input_allowed(10));
        assert_eq!(limit.check(9, 15, &history), None, "not confirmed yet");
        assert_eq!(limit.check(10, 15, &history), None, "settle starts");
        assert_eq!(limit.check(30, 74, &history), None);
        let line = limit.check(30, 75, &history);
        assert_eq!(
            line.as_deref(),
            Some("ram_crc=0000abcd input_crc=00000007 frames=10")
        );
        assert_eq!(limit.check(40, 90, &history), None, "printed once");
        let mut watcher = FramesLimit::new(10, 0);
        assert!(
            watcher.check(10, 10, &history).is_some(),
            "spectator: no settle"
        );
    }
}
