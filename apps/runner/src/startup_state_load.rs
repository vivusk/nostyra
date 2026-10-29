//! `--load-state N|auto`: loads the slot (or the `auto` resume point) right after the
//! core and ROM are loaded, before the first presented frame (windowed and headless).
//! Snes9x accepts a state right after `retro_load_game`; a core with the
//! `MUST_INITIALIZE` quirk may refuse until it has run, and so does a core whose states
//! are `Serialized` (Mupen64Plus-Next refuses before its first frame without declaring
//! the quirk), so their core rejections are retried after each frame, up to
//! [`MAX_RETRY_FRAMES`]. A missing or incompatible slot fails
//! at once: the game simply starts from power-on and the error is shown/logged (never
//! fatal). A failed resume says "Could not resume" (no slot number: the launcher
//! already offered it as Continue).
//!
//! A core whose state size is re-queried (PPSSPP) accepts any state before it has
//! booted and ignores the later result, so a bad resume point would say "Resumed":
//! its load waits until the core booted (its state size changed, see [`BootGate`]),
//! then gets exactly one attempt whose result is the real one.
//!
//! Achievement hardcore skips the load before anything else (no file read, no core
//! call): the game boots fresh with [`STATE_LOAD_REFUSED`], never a failed exit.

use std::time::Instant;

use ipc_protocol::StateSlotId;
use libretro_host::{CoreSession, HostError};
use system_catalog::SaveStateQuality;

use crate::game_save_data::{GameSaveData, STATE_LOADED};
use crate::hardcore_gate::STATE_LOAD_REFUSED;
use crate::save_state_slots::{failure_status, STATES_NEED_HW_CONTEXT};
use crate::startup_boot_gate::BootGate;

/// Frames a core that is not ready yet gets before the load is given up.
pub const MAX_RETRY_FRAMES: u32 = 60;

/// Status after the resume point loaded.
pub const RESUMED: &str = "Resumed";
/// Status after the resume point failed to load (game starts fresh).
pub const COULD_NOT_RESUME: &str = "Could not resume";

/// Result of a finished startup load: whether it loaded, the slot status text and
/// the state's achievement progress (when loaded).
pub struct StartupLoadOutcome {
    pub loaded: bool,
    pub status: String,
    pub progress: Option<String>,
}

/// What one startup attempt needs from the game (a fake in tests).
trait StartupTarget {
    /// Achievement hardcore: loads refused.
    fn hardcore_active(&self) -> bool;
    fn has_state(&self, slot: StateSlotId) -> bool;
    /// False: no state call may reach the core (`hw_render_required` core, no context).
    fn states_available(&self) -> bool;
    fn state_size_requery(&self) -> bool;
    fn current_state_size(&mut self) -> usize;
    fn must_initialize(&self) -> bool;
    fn load_slot(&mut self, slot: StateSlotId) -> anyhow::Result<Option<String>>;
}

struct Live<'a> {
    saves: &'a GameSaveData,
    session: &'a mut CoreSession,
}

impl StartupTarget for Live<'_> {
    fn hardcore_active(&self) -> bool {
        self.saves.hardcore_active()
    }
    fn has_state(&self, slot: StateSlotId) -> bool {
        self.saves.has_state(slot)
    }
    fn states_available(&self) -> bool {
        self.session.states_available()
    }
    fn state_size_requery(&self) -> bool {
        self.session.state_size_requery()
    }
    fn current_state_size(&mut self) -> usize {
        self.session.current_state_size()
    }
    fn must_initialize(&self) -> bool {
        self.session.serialization_quirks().must_initialize()
    }
    fn load_slot(&mut self, slot: StateSlotId) -> anyhow::Result<Option<String>> {
        self.saves.load_slot(self.session, slot)
    }
}

pub struct StartupStateLoad {
    slot: StateSlotId,
    retries_left: u32,
    /// Retry core rejections even without `MUST_INITIALIZE` (`Serialized` states).
    retry_rejections: bool,
    boot: BootGate,
}

impl StartupStateLoad {
    /// `quality`: the core's save-state quality (registry).
    pub fn new(slot: StateSlotId, quality: SaveStateQuality) -> Self {
        Self {
            slot,
            retries_left: MAX_RETRY_FRAMES,
            retry_rejections: quality == SaveStateQuality::Serialized,
            boot: BootGate::default(),
        }
    }

    pub fn slot(&self) -> StateSlotId {
        self.slot
    }

    /// One attempt; call before the first frame, then after each frame while it
    /// returns `None` (core not ready yet).
    pub fn attempt(
        &mut self,
        saves: &GameSaveData,
        session: &mut CoreSession,
    ) -> Option<StartupLoadOutcome> {
        self.attempt_on(&mut Live { saves, session })
    }

    fn attempt_on(&mut self, game: &mut impl StartupTarget) -> Option<StartupLoadOutcome> {
        if game.hardcore_active() {
            tracing::warn!(slot = %self.slot, "--load-state skipped: hardcore mode");
            return Some(StartupLoadOutcome {
                loaded: false,
                status: STATE_LOAD_REFUSED.to_string(),
                progress: None,
            });
        }
        if !game.has_state(self.slot) {
            tracing::error!(slot = %self.slot, "--load-state: slot is empty");
            return Some(self.failed("Empty slot"));
        }
        if !game.states_available() {
            tracing::error!(slot = %self.slot, "--load-state: save states need hardware rendering");
            return Some(self.failed(STATES_NEED_HW_CONTEXT));
        }
        let requery = game.state_size_requery();
        if requery && !self.boot.passed(game.current_state_size(), Instant::now()) {
            return None; // not booted yet: a load now would be accepted blindly
        }
        let err = match game.load_slot(self.slot) {
            Ok(progress) => {
                let status = match self.slot {
                    StateSlotId::Auto => RESUMED,
                    StateSlotId::Numbered(_) => STATE_LOADED,
                };
                return Some(StartupLoadOutcome {
                    loaded: true,
                    status: status.to_string(),
                    progress,
                });
            }
            Err(err) => err,
        };
        let core_rejected = matches!(
            err.downcast_ref::<HostError>(),
            Some(HostError::UnserializeFailed)
        );
        let must_initialize = game.must_initialize();
        let not_ready =
            core_rejected && retries_rejection(requery, self.retry_rejections, must_initialize);
        if not_ready && self.retries_left > 0 {
            self.retries_left -= 1;
            return None;
        }
        tracing::error!(slot = %self.slot, "--load-state failed: {err:#}");
        Some(self.failed(&failure_status(&err, "Load failed")))
    }

    /// Failed outcome; the resume point always reports [`COULD_NOT_RESUME`].
    fn failed(&self, numbered_status: &str) -> StartupLoadOutcome {
        let status = match self.slot {
            StateSlotId::Auto => COULD_NOT_RESUME,
            StateSlotId::Numbered(_) => numbered_status,
        };
        StartupLoadOutcome {
            loaded: false,
            status: status.to_string(),
            progress: None,
        }
    }
}

/// Whether a core rejection means "not ready yet, retry after the next frame". After
/// the boot wait a re-query core's answer is final: exactly one attempt.
fn retries_rejection(requery: bool, serialized: bool, must_initialize: bool) -> bool {
    !requery && (serialized || must_initialize)
}

#[cfg(test)]
mod attempt_tests;
