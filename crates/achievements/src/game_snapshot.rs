//! Per-game achievement snapshot the runner writes and the launcher's Achievements tab
//! reads (`<data>/achievements/games/<content id>.json`): the launcher has no live RA
//! client, so this file is its only view of the set and unlock state.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::achievement_info::AchievementInfo;

/// Current schema; readers accept files with a newer value as long as fields parse.
pub const SNAPSHOT_SCHEMA: u32 = 1;

/// One game's set as last seen by the runner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSnapshot {
    #[serde(default = "schema_default")]
    pub schema: u32,
    pub ra_game_id: u32,
    pub title: String,
    /// Game icon name (`game_icon_path` / `game_icon_url`).
    pub icon_badge: String,
    /// Mode of the session that wrote the snapshot.
    pub hardcore: bool,
    pub achievements: Vec<SnapshotAchievement>,
    /// Unix seconds of the write.
    pub updated_at: i64,
}

/// One achievement row of a [`GameSnapshot`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotAchievement {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    /// Badge name (`badge_path` / `badge_url`).
    pub badge: String,
    /// Unlocked in either mode.
    pub unlocked: bool,
    /// Unlocked in hardcore.
    #[serde(default)]
    pub unlocked_hardcore: bool,
    /// Unix seconds; 0 while locked.
    pub unlock_time: i64,
    /// Progress text such as `"5/10"`; empty when not measured.
    #[serde(default)]
    pub measured_progress: String,
    /// `1` core, `2` unofficial.
    pub category: u8,
}

fn schema_default() -> u32 {
    SNAPSHOT_SCHEMA
}

/// `RC_CLIENT_ACHIEVEMENT_UNLOCKED_HARDCORE`.
const UNLOCKED_HARDCORE_BIT: u8 = 2;

impl SnapshotAchievement {
    /// Row from an rc_client achievement.
    pub fn from_info(info: &AchievementInfo) -> Self {
        Self {
            id: info.id,
            title: info.title.clone(),
            description: info.description.clone(),
            points: info.points,
            badge: info.badge_name.clone(),
            unlocked: info.unlocked != 0,
            unlocked_hardcore: info.unlocked & UNLOCKED_HARDCORE_BIT != 0,
            unlock_time: info.unlock_time,
            measured_progress: info.measured_progress.clone(),
            category: info.category,
        }
    }
}

/// Snapshot file of one game (`content_id` = the `ipc_protocol` content id).
pub fn snapshot_path(data_dir: &Path, content_id: &str) -> PathBuf {
    data_dir
        .join("achievements")
        .join("games")
        .join(format!("{content_id}.json"))
}

/// Writes atomically (the launcher may read while the runner writes).
pub fn save_game_snapshot(
    data_dir: &Path,
    content_id: &str,
    snapshot: &GameSnapshot,
) -> std::io::Result<()> {
    let path = snapshot_path(data_dir, content_id);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(snapshot).map_err(std::io::Error::other)?;
    config_layers::write_atomically(&path, &json)
}

/// Reads a snapshot; `Ok(None)` when the game was never played while logged in.
pub fn load_game_snapshot(
    data_dir: &Path,
    content_id: &str,
) -> std::io::Result<Option<GameSnapshot>> {
    match std::fs::read(snapshot_path(data_dir, content_id)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(std::io::Error::other),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use super::*;

    #[test]
    fn round_trips_and_maps_unlock_bits() {
        let dir = tempfile::tempdir().unwrap();
        let info = AchievementInfo {
            id: 101,
            title: "Ach".into(),
            badge_name: "00101".into(),
            unlocked: 3,
            unlock_time: 1_700_000_000,
            category: 1,
            points: 5,
            ..Default::default()
        };
        let snapshot = GameSnapshot {
            schema: SNAPSHOT_SCHEMA,
            ra_game_id: 1234,
            title: "Game".into(),
            icon_badge: "085573".into(),
            hardcore: false,
            achievements: vec![SnapshotAchievement::from_info(&info)],
            updated_at: 1_700_000_100,
        };
        assert!(snapshot.achievements[0].unlocked && snapshot.achievements[0].unlocked_hardcore);
        assert_eq!(load_game_snapshot(dir.path(), "abc").unwrap(), None);
        save_game_snapshot(dir.path(), "abc", &snapshot).unwrap();
        assert_eq!(
            load_game_snapshot(dir.path(), "abc").unwrap(),
            Some(snapshot)
        );
        assert!(dir.path().join("achievements/games/abc.json").is_file());
    }

    #[test]
    fn older_files_without_optional_fields_parse() {
        let json = r#"{"ra_game_id":1,"title":"T","icon_badge":"","hardcore":true,
            "achievements":[{"id":1,"title":"a","description":"d","points":5,"badge":"1",
            "unlocked":false,"unlock_time":0,"category":1}],"updated_at":0}"#;
        let parsed: GameSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.schema, SNAPSHOT_SCHEMA);
        assert!(!parsed.achievements[0].unlocked_hardcore);
    }
}
