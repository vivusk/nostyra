//! L6: PPSSPP boots on its own thread after `retro_load_game` and accepts any state
//! before that, only to drop it at boot. For re-query cores a slot load (F4, pause
//! menu) is refused with "Core not ready" until the core booted (its state size
//! changed, or [`BOOT_WAIT`](crate::startup_boot_gate::BOOT_WAIT) passed).

use std::cell::RefCell;
use std::time::Instant;

use libretro_host::CoreSession;

use crate::startup_boot_gate::BootGate;

/// [`crate::game_save_data::GameSaveData::load_state`] status before boot.
pub const CORE_NOT_READY: &str = "Core not ready";

#[derive(Debug, Default)]
pub struct PreBootLoadGuard {
    /// `None` for cores whose states are valid from the first frame.
    gate: Option<RefCell<BootGate>>,
}

impl PreBootLoadGuard {
    /// Takes the pre-boot state size right after the load (not for a session without
    /// states: its loads are refused anyway, see `states_available`).
    pub fn new(session: &mut CoreSession) -> Self {
        if !session.state_size_requery() || !session.states_available() {
            return Self::default();
        }
        let mut gate = BootGate::default();
        gate.passed(session.current_state_size(), Instant::now());
        Self {
            gate: Some(RefCell::new(gate)),
        }
    }

    /// Whether a state may be loaded now.
    pub fn ready(&self, session: &mut CoreSession) -> bool {
        self.gate.as_ref().is_none_or(|gate| {
            let size = session.current_state_size();
            gate.borrow_mut().passed(size, Instant::now())
        })
    }
}
