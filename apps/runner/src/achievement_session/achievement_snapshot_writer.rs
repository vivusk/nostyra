//! Writes `<data>/achievements/games/<content id>.json` (the launcher's only view of
//! a game's set, M15 phase 06 tab) after the set loads and after every unlock:
//! title, icon, every achievement with its badge, unlock state and progress.

use std::path::{Path, PathBuf};

use achievements::{
    save_game_snapshot, AchievementClient, GameSnapshot, GameSummary, SnapshotAchievement,
};

/// `achievement_list` category: core + unofficial (the snapshot keeps the category).
const ALL_CATEGORIES: i32 = 3;
/// `achievement_list` grouping by lock state (any grouping lists each achievement once).
const BY_LOCK_STATE: i32 = 0;

/// Where this session's snapshot goes.
pub(super) struct SnapshotTarget {
    pub(super) data_root: PathBuf,
    pub(super) content_id: String,
}

impl SnapshotTarget {
    pub(super) fn new(data_root: &Path, content_id: &str) -> Self {
        Self {
            data_root: data_root.to_path_buf(),
            content_id: content_id.to_owned(),
        }
    }

    /// Writes the snapshot of the loaded game (atomic) and returns its unlock totals;
    /// `None` when no game is loaded. A write error is logged (the game goes on).
    pub(super) fn write(&self, client: &AchievementClient) -> Option<GameSummary> {
        if !client.is_game_loaded() {
            return None;
        }
        let game = client.game_info()?;
        let achievements = client
            .achievement_list(ALL_CATEGORIES, BY_LOCK_STATE)
            .iter()
            .flat_map(|bucket| {
                bucket
                    .achievements
                    .iter()
                    .map(SnapshotAchievement::from_info)
            })
            .collect();
        let snapshot = GameSnapshot {
            schema: achievements::SNAPSHOT_SCHEMA,
            ra_game_id: game.id,
            title: game.title,
            icon_badge: game.badge_name,
            hardcore: client.hardcore_enabled(),
            achievements,
            updated_at: chrono::Utc::now().timestamp(),
        };
        if let Err(err) = save_game_snapshot(&self.data_root, &self.content_id, &snapshot) {
            tracing::warn!(%err, "cannot write the achievement snapshot");
        }
        Some(client.user_game_summary())
    }
}
