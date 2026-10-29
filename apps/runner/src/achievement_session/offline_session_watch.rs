//! The session side of offline starts (M15 follow-up): the transport's
//! [`OfflineCacheState`] is checked on every pump (two atomic loads). Any answer from
//! the cache makes the session softcore (hardcore needs server-verified answers; a
//! no-op while hardcore is hidden), and a known game's set or session from the cache
//! tells the player once that unlocks are kept and will sync.

use std::sync::Arc;

use achievements::OfflineCacheState;

use super::AchievementSession;

/// Popup when a cached set / session is in use (once per session).
pub(super) const OFFLINE_SYNC_NOTICE: &str =
    "RetroAchievements: offline — unlocks will sync when online";

/// The cache flags of this session's transport and what was already done about them.
#[derive(Default)]
pub(super) struct OfflineSessionWatch {
    state: Option<Arc<OfflineCacheState>>,
    softcore_done: bool,
    notice_done: bool,
}

impl OfflineSessionWatch {
    pub(super) fn new(state: Option<Arc<OfflineCacheState>>) -> Self {
        Self {
            state,
            ..Self::default()
        }
    }
}

impl AchievementSession {
    /// Reacts to newly served cache answers: softcore first, then the sync popup.
    pub(super) fn follow_offline_cache(&mut self) {
        let Some(state) = self.offline.state.clone() else {
            return;
        };
        if !self.offline.softcore_done && state.any_served() {
            self.offline.softcore_done = true;
            self.drop_to_softcore("offline");
        }
        if !self.offline.notice_done && state.set_served() {
            self.offline.notice_done = true;
            self.router.notify(OFFLINE_SYNC_NOTICE.into());
        }
    }
}
