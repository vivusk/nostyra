//! Per-core session rules the runner switches on from the registry (`CoreSpec`):
//!
//! - **States need a hardware context** (`hw_render_required` cores): Dolphin on its
//!   Null / Software renderer segfaults inside `retro_serialize_size` (M14 spike), so
//!   while the session has no hardware context every state entry point
//!   (`serialize_into`, `unserialize_from`, `current_state_size`) answers without a
//!   core call: [`HostError::StatesNeedHwContext`] / size 0.
//! - **Library kept loaded** (`SessionConfig::keep_library_loaded`, set from the
//!   load on): `retro_unload_game` and `retro_deinit` still run at drop, but the
//!   library is never freed (Dolphin: `FreeLibrary` hangs after a failed boot, and
//!   after any `retro_unserialize` the session thread's exit hangs).
//! - **Abandon**: a session the runner refuses after a successful load (Dolphin fell
//!   back to its black Null renderer) ends without any core call and stays mapped;
//!   the process exits right after (exit 6, `process_hard_exit`).

use crate::core_session::CoreSession;
use crate::host_error::{HostError, HostResult};

impl CoreSession {
    /// Refuse every state call while the session has no hardware context (call after
    /// `load`; the context exists from then on or never).
    pub fn require_hw_context_for_states(&mut self, required: bool) {
        self.ctx_mut().states_need_hw_context = required;
    }

    /// Whether save states may reach the core (see the module doc).
    pub fn states_available(&self) -> bool {
        !self.ctx().states_need_hw_context || self.uses_hw_context()
    }

    /// `Err(StatesNeedHwContext)` when [`Self::states_available`] is false.
    pub(crate) fn ensure_states_available(&self) -> HostResult<()> {
        match self.states_available() {
            true => Ok(()),
            false => Err(HostError::StatesNeedHwContext),
        }
    }

    /// Ends the session at drop without `retro_unload_game` / `retro_deinit` and
    /// keeps the library mapped: for a core the runner refuses right after its load.
    pub fn abandon_core(&mut self) {
        self.game_loaded = false;
        self.initialized = false;
        self.core.keep_loaded();
    }
}

#[cfg(test)]
#[path = "core_session_core_rules_tests.rs"]
mod tests;
