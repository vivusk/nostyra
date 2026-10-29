//! Achievement hardcore and the game's states: the session's `HardcoreGate` is
//! shared with the slots, whose `load` refuses first (the chokepoint); F4 / the menu
//! check it even before the pre-boot guard, which queries the core. Saving (F1, menu,
//! auto-save on exit) stays allowed.

use super::GameSaveData;
use crate::hardcore_gate::{HardcoreGate, STATE_LOAD_REFUSED};

impl GameSaveData {
    /// Shares the achievement session's hardcore state with the state slots.
    pub fn set_hardcore_gate(&mut self, gate: HardcoreGate) {
        self.states.set_hardcore_gate(gate);
    }

    /// Whether state loads are refused now (startup `--load-state` skips its load).
    pub fn hardcore_active(&self) -> bool {
        self.states.hardcore_active()
    }

    /// The load status while hardcore refuses loads.
    pub(super) fn hardcore_refusal(&self) -> Option<String> {
        self.hardcore_active()
            .then(|| STATE_LOAD_REFUSED.to_string())
    }
}
