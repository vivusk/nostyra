//! Shared handle between a running sync job and the launcher: the job's phase (so
//! Play can decide "cancel the transfer" vs "wait for the short apply pass") and the
//! cancel flag the executor checks between files and chunks.
//!
//! Phase and cancel flag live in **one** atomic, so "cancel, then look at the phase"
//! ([`SyncProgress::cancel_and_phase`]) and "enter `Applying` unless cancelled"
//! ([`SyncProgress::enter_applying`]) are ordered: once Play has cancelled a job seen
//! in `Transferring`, the worker can never start an apply pass afterwards.

use std::sync::atomic::{AtomicU8, Ordering};

/// What the job is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPhase {
    Idle,
    /// Network / staging only: local saves untouched, safe to cancel.
    Transferring,
    /// Moving files in the save folders (ms-scale): not cancellable, wait for it.
    Applying,
}

/// Low two bits: the phase; this bit: cancelled.
const CANCEL_BIT: u8 = 0b100;
const PHASE_MASK: u8 = 0b011;

/// Phase + cancel flag of one sync job (share it behind an `Arc`).
#[derive(Debug, Default)]
pub struct SyncProgress {
    state: AtomicU8,
}

impl SyncProgress {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current phase.
    pub fn phase(&self) -> SyncPhase {
        phase_of(self.state.load(Ordering::SeqCst))
    }

    pub(crate) fn set_phase(&self, phase: SyncPhase) {
        let value = phase_bits(phase);
        self.update(|old| Some((old & CANCEL_BIT) | value));
    }

    /// Sets the phase from another crate's tests (a job simulated without a sync).
    #[cfg(any(test, feature = "test-support"))]
    pub fn set_phase_for_tests(&self, phase: SyncPhase) {
        self.set_phase(phase);
    }

    /// Switches to `Applying` unless the job was cancelled (then `false`, phase kept):
    /// the executor's last cancel check before it touches a save folder.
    pub(crate) fn enter_applying(&self) -> bool {
        self.update(|old| (old & CANCEL_BIT == 0).then_some(phase_bits(SyncPhase::Applying)))
    }

    /// Compare-exchange loop: applies `next` to the state; `false` when it declines.
    fn update(&self, next: impl Fn(u8) -> Option<u8>) -> bool {
        let mut old = self.state.load(Ordering::SeqCst);
        loop {
            let Some(new) = next(old) else {
                return false;
            };
            match self
                .state
                .compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return true,
                Err(current) => old = current,
            }
        }
    }

    /// Asks the job to stop at the next file / chunk boundary of a transfer.
    pub fn cancel(&self) {
        self.cancel_and_phase();
    }

    /// Cancels and returns the phase at that instant: `Applying` = an apply pass is
    /// running (wait for it to end); anything else = no apply pass will start.
    pub fn cancel_and_phase(&self) -> SyncPhase {
        phase_of(self.state.fetch_or(CANCEL_BIT, Ordering::SeqCst))
    }

    /// Whether [`Self::cancel`] was called.
    pub fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::SeqCst) & CANCEL_BIT != 0
    }
}

fn phase_bits(phase: SyncPhase) -> u8 {
    match phase {
        SyncPhase::Idle => 0,
        SyncPhase::Transferring => 1,
        SyncPhase::Applying => 2,
    }
}

fn phase_of(state: u8) -> SyncPhase {
    match state & PHASE_MASK {
        1 => SyncPhase::Transferring,
        2 => SyncPhase::Applying,
        _ => SyncPhase::Idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_and_cancel_round_trip() {
        let progress = SyncProgress::new();
        assert_eq!(progress.phase(), SyncPhase::Idle);
        for phase in [
            SyncPhase::Transferring,
            SyncPhase::Applying,
            SyncPhase::Idle,
        ] {
            progress.set_phase(phase);
            assert_eq!(progress.phase(), phase);
        }
        assert!(!progress.is_cancelled());
        progress.cancel();
        assert!(progress.is_cancelled());
        assert_eq!(progress.phase(), SyncPhase::Idle, "cancel keeps the phase");
    }

    #[test]
    fn no_apply_pass_starts_after_a_cancel() {
        let progress = SyncProgress::new();
        progress.set_phase(SyncPhase::Transferring);
        assert!(progress.enter_applying());
        assert_eq!(progress.cancel_and_phase(), SyncPhase::Applying);
        progress.set_phase(SyncPhase::Transferring);
        assert!(progress.is_cancelled(), "set_phase keeps the cancel bit");
        assert!(!progress.enter_applying());
        assert_eq!(progress.phase(), SyncPhase::Transferring);
        assert_eq!(progress.cancel_and_phase(), SyncPhase::Transferring);
    }
}
