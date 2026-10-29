//! The one runtime value every RetroAchievements hardcore rule keys off (M15 phase
//! 04): on when this session started in hardcore (the setting, the hidden UI flag and
//! no disallowed core option) and was not dropped to softcore since (unknown disc,
//! token rejected). Shared by the achievement session (owner, the only one that
//! lifts it), the state slots (load refusal) and rewind; single thread, so a shared
//! `Cell` is enough. Without a session every holder keeps the default (inactive).

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

/// Status of every refused state load (F4, menu, `--load-state`, resume point).
pub const STATE_LOAD_REFUSED: &str = "Loading states is disabled in hardcore mode";
/// Pause-menu reason on the Rewind row while hardcore is active.
pub const REWIND_REFUSED: &str = "Disabled in hardcore";
/// On-screen text while the rewind input is held in hardcore.
pub const REWIND_REFUSED_STATUS: &str = "Rewind is disabled in hardcore mode";

/// Shared hardcore state of one session (clones see the same value).
#[derive(Clone, Debug, Default)]
pub struct HardcoreGate(Rc<Cell<bool>>);

impl HardcoreGate {
    pub fn new(active: bool) -> Self {
        Self(Rc::new(Cell::new(active)))
    }

    /// Whether the hardcore rules apply right now.
    pub fn active(&self) -> bool {
        self.0.get()
    }

    /// Ends hardcore for the rest of the session (gates lift everywhere at once);
    /// returns whether it was active.
    pub fn lift(&self) -> bool {
        self.0.replace(false)
    }
}

/// Error of a state load refused in hardcore, raised before any file read or core
/// call; `failure_status` turns it into [`STATE_LOAD_REFUSED`].
#[derive(Debug)]
pub struct HardcoreLoadRefused;

impl fmt::Display for HardcoreLoadRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(STATE_LOAD_REFUSED)
    }
}

impl std::error::Error for HardcoreLoadRefused {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_share_the_value_and_lift_once() {
        let gate = HardcoreGate::new(true);
        let holder = gate.clone();
        assert!(holder.active());
        assert!(gate.lift(), "was active");
        assert!(!holder.active(), "every clone sees the lift");
        assert!(!gate.lift());
        assert!(!HardcoreGate::default().active(), "no session: inactive");
        assert_eq!(HardcoreLoadRefused.to_string(), STATE_LOAD_REFUSED);
    }
}
