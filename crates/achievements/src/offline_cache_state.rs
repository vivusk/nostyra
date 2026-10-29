//! What the offline cache transport served (M15 follow-up), shared between the HTTP
//! worker that serves answers and the session thread that reacts to them (softcore,
//! the "unlocks will sync" popup). Plain atomics: written rarely, read every pump.

use std::sync::atomic::{AtomicBool, Ordering};

/// Served-from-cache flags of one session's transport.
#[derive(Debug, Default)]
pub struct OfflineCacheState {
    any_served: AtomicBool,
    set_served: AtomicBool,
}

impl OfflineCacheState {
    /// Some answer (login, set or session) came from the cache: hardcore must not stay
    /// on, since nothing was verified by the server.
    pub fn any_served(&self) -> bool {
        self.any_served.load(Ordering::Relaxed)
    }

    /// A known game's set (`GameId > 0`) or session came from the cache: unlocks are
    /// counted and journaled, and will sync later. A cached login alone does not count.
    pub fn set_served(&self) -> bool {
        self.set_served.load(Ordering::Relaxed)
    }

    /// One answer was served from the cache; `known_game` for a known game's set or session.
    pub(crate) fn mark_served(&self, known_game: bool) {
        self.any_served.store(true, Ordering::Relaxed);
        if known_game {
            self.set_served.store(true, Ordering::Relaxed);
        }
    }
}
