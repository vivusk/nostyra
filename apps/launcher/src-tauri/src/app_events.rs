//! Events pushed to the UI. Names are mirrored in `src/api/tauri-events.ts`.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const SCAN_PROGRESS: &str = "scan-progress";
pub const SCAN_FINISHED: &str = "scan-finished";
pub const CORE_DOWNLOAD_PROGRESS: &str = "core-download-progress";
pub const CORE_DOWNLOAD_FINISHED: &str = "core-download-finished";
pub const SESSION_STARTED: &str = "session-started";
pub const SESSION_LAUNCH_TIMEOUT: &str = "session-launch-timeout";
pub const SESSION_ENDED: &str = "session-ended";
pub const ARTWORK_PROGRESS: &str = "artwork-progress";
pub const ARTWORK_UPDATED: &str = "artwork-updated";
pub const ARTWORK_FINISHED: &str = "artwork-finished";
/// Settings → Data folder move: `{ done, total }` files copied (the app restarts after).
pub const DATA_FOLDER_MOVE_PROGRESS: &str = "data-folder-move-progress";
/// A game's achievement snapshot changed (set loaded / unlock): `{ gameId }`.
pub const ACHIEVEMENTS_CHANGED: &str = "achievements-changed";
/// The stored RetroAchievements login was rejected and deleted (no payload).
pub const RA_LOGGED_OUT: &str = "ra-logged-out";

/// Live scan state; also returned by `get_scan_progress` for late subscribers.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressPayload {
    pub folder_path: String,
    pub files_total: usize,
    pub files_done: usize,
    pub current_path: Option<String>,
}

/// End of a scan over all folders.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanFinishedPayload {
    pub added: usize,
    pub updated: usize,
    pub missing: usize,
    pub relinked: usize,
    pub skipped: usize,
    pub failed_files: usize,
    /// Why files were not added (first [`MAX_SCAN_PROBLEMS`]; `failed_files` counts all).
    pub failures: Vec<ScanProblem>,
    pub cancelled: bool,
    /// A folder that could not be scanned at all (e.g. drive unplugged).
    pub error: Option<String>,
}

/// Scan problems listed in the "files not added" dialog at most.
pub const MAX_SCAN_PROBLEMS: usize = 100;

/// One file a scan could not add, with a readable reason.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProblem {
    pub path: String,
    pub reason: String,
}

/// Live artwork job state; also returned by `get_artwork_progress`.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkProgressPayload {
    pub done: usize,
    pub total: usize,
    pub current_title: Option<String>,
}

/// End of an artwork job (counts are per image kind, summed over rerun passes).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkFinishedPayload {
    pub cached: usize,
    pub not_found: usize,
    pub failed: usize,
    /// Offline / network trouble: nothing stored, retried by the next job.
    pub transient: usize,
    /// The user cancelled (a dead-network stop is not a cancel).
    pub cancelled: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreDownloadProgressPayload {
    pub core_id: &'static str,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreDownloadFinishedPayload {
    pub core_id: &'static str,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPayload {
    pub game_id: i64,
}

/// A finished game session; `error` set when the runner failed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEndedPayload {
    pub game_id: i64,
    pub error: Option<SessionErrorPayload>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionErrorPayload {
    pub kind: runner_session::SessionErrorKind,
    /// Readable sentence for the toast.
    pub message: String,
    /// Runner error / stderr tail, for "details".
    pub detail: String,
}

/// Emits to all windows; a failed emit (window closing) is only logged.
pub fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(err) = app.emit(event, payload) {
        tracing::warn!(event, %err, "cannot emit event");
    }
}
