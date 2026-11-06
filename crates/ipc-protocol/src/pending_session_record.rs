//! The pending play-session file a runner (or the web edition) leaves in
//! `sessions/pending/` for the desktop launcher to import.

use serde::{Deserialize, Serialize};

/// Content of a pending session file. Survives the launcher closing mid-game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingSessionRecord {
    /// Exactly the `--rom` argument, which the launcher takes from `games.rom_path`
    /// (the web edition writes its own `web:<folder>/<path>` value, which never
    /// matches a desktop row: [`Self::content_sha1`] does then).
    pub rom_path: String,
    /// Unix seconds when the session started.
    pub started_at: i64,
    pub play_seconds: u64,
    pub exit_code: i32,
    /// Game content sha1 (`games.sha1`, upper hex), written by the web edition so the
    /// desktop can count the session when `rom_path` is not one of its rows. Absent in
    /// runner records (the field is then not written).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha1: Option<String>,
}

/// Unique, sortable file name: `<started_ms>-<pid>.json` (web: `<started_ms>-web.json`).
pub fn pending_session_file_name(started_unix_ms: u128, pid: u32) -> String {
    format!("{started_unix_ms}-{pid}.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_json_round_trip_and_old_files() {
        assert_eq!(
            pending_session_file_name(1_700_000_000_123, 42),
            "1700000000123-42.json"
        );
        let record = PendingSessionRecord {
            rom_path: "E:/ROMs/a.sfc".into(),
            started_at: 1_700_000_000,
            play_seconds: 90,
            exit_code: 0,
            content_sha1: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("content_sha1"), "{json}");
        assert_eq!(
            serde_json::from_str::<PendingSessionRecord>(&json).unwrap(),
            record
        );
        let web = PendingSessionRecord {
            content_sha1: Some("6B47BB75D16514B6A476AA0C73A683A2A4C18765".into()),
            ..record
        };
        let json = serde_json::to_string(&web).unwrap();
        assert_eq!(
            serde_json::from_str::<PendingSessionRecord>(&json).unwrap(),
            web
        );
    }
}
