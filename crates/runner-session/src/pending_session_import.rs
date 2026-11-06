//! Imports play-session files the runner (or the web edition) left in
//! `<data>/sessions/pending/` into `play_sessions`, then removes them. Runs after each
//! session and at launcher start, so play time survives the launcher closing
//! mid-game. Idempotent: the file name is the row's `source_key` (`INSERT OR IGNORE`),
//! and a file is removed only after its row is committed.
//!
//! A web record names its game by `rom_path` = `web:<folder>/<path>`, which is never a
//! desktop row: it is matched by its `content_sha1` instead (`games.sha1`). One that
//! matches no game yet is kept for [`WEB_RECORD_WAIT_S`] (the desktop may not have
//! scanned that ROM), then dropped.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ipc_protocol::{
    pending_sessions_dir, remove_stale_browser_swap_files, PendingSessionRecord,
    STALE_BROWSER_SWAP_AGE,
};
use storage_db::{
    find_game_id_by_rom_path, find_game_id_by_sha1, record_play_session, Connection, NewPlaySession,
};

/// How long a record carrying a content sha1 waits for its game to be scanned.
pub const WEB_RECORD_WAIT_S: i64 = 30 * 24 * 60 * 60;

/// Runner Started vs launcher receipt can differ by process start latency.
const SAME_SESSION_TOLERANCE_S: i64 = 120;

/// What an import pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingImportSummary {
    pub imported: usize,
    /// Already recorded earlier (file left behind by an interrupted import).
    pub duplicates: usize,
    /// ROM path (and content sha1) not in the library (game removed): dropped.
    pub unknown_game: usize,
    /// Records with a content sha1 no game has yet: kept for a later pass.
    pub waiting: usize,
    /// Unreadable/corrupt files: dropped.
    pub invalid: usize,
    /// `(game_id, started_at)` of every file now in `play_sessions` (imported or duplicate).
    pub recorded: Vec<(i64, i64)>,
}

impl PendingImportSummary {
    /// Whether the runner's own record of a session (same game, started within the
    /// tolerance) is in the database, so the launcher must not add its measurement.
    /// Keyed to the session, so an unrelated leftover file cannot mask a lost one.
    pub fn covers_session(&self, game_id: i64, started_at: i64) -> bool {
        self.recorded
            .iter()
            .any(|&(id, at)| id == game_id && (at - started_at).abs() <= SAME_SESSION_TOLERANCE_S)
    }
}

/// Database failure aborts the pass (files stay for the next attempt).
#[derive(Debug, thiserror::Error)]
pub enum PendingImportError {
    #[error("cannot read pending sessions in {path}: {source}")]
    ReadDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Storage(#[from] storage_db::StorageError),
}

/// Imports every `*.json` pending session under `data_root`. Missing folder = nothing
/// to do. Stale browser swap files (an interrupted web write) are removed on the way.
pub fn import_pending_sessions(
    conn: &Connection,
    data_root: &Path,
) -> Result<PendingImportSummary, PendingImportError> {
    let dir = pending_sessions_dir(data_root);
    let mut summary = PendingImportSummary::default();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(summary),
        Err(source) => return Err(PendingImportError::ReadDir { path: dir, source }),
    };
    let now = unix_now();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        // `*.json.tmp` / `*.json.crswap` = a write in progress (or interrupted).
        if !file_name.ends_with(".json") {
            continue;
        }
        let record = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<PendingSessionRecord>(&bytes).ok());
        let done = match record {
            None => {
                tracing::warn!(file = %path.display(), "dropping unreadable pending session");
                summary.invalid += 1;
                true
            }
            Some(record) => import_record(conn, &record, file_name, now, &mut summary)?,
        };
        if !done {
            continue;
        }
        if let Err(err) = std::fs::remove_file(&path) {
            // Harmless: the next pass sees a duplicate and retries the removal.
            tracing::warn!(file = %path.display(), %err, "cannot remove pending session");
        }
    }
    remove_stale_browser_swap_files(&dir, STALE_BROWSER_SWAP_AGE);
    Ok(summary)
}

/// Records one session; `false` = keep the file for a later pass (its game is not
/// in the library yet).
fn import_record(
    conn: &Connection,
    record: &PendingSessionRecord,
    file_name: &str,
    now: i64,
    summary: &mut PendingImportSummary,
) -> Result<bool, PendingImportError> {
    let content_sha1 = record
        .content_sha1
        .as_deref()
        .filter(|sha1| !sha1.is_empty());
    let mut game_id = find_game_id_by_rom_path(conn, &record.rom_path)?;
    if let (None, Some(sha1)) = (game_id, content_sha1) {
        game_id = find_game_id_by_sha1(conn, sha1)?;
    }
    let Some(game_id) = game_id else {
        if content_sha1.is_some() && now.saturating_sub(record.started_at) < WEB_RECORD_WAIT_S {
            summary.waiting += 1;
            return Ok(false);
        }
        tracing::info!(rom = %record.rom_path, "pending session for unknown game dropped");
        summary.unknown_game += 1;
        return Ok(true);
    };
    let session = NewPlaySession {
        game_id,
        started_at: record.started_at,
        duration_s: i64::try_from(record.play_seconds).unwrap_or(i64::MAX),
        exit_code: Some(record.exit_code),
        source_key: Some(file_name.to_string()),
    };
    if record_play_session(conn, &session)? {
        summary.imported += 1;
    } else {
        summary.duplicates += 1;
    }
    summary.recorded.push((game_id, record.started_at));
    Ok(true)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_only_the_same_game_near_the_same_start() {
        let summary = PendingImportSummary {
            recorded: vec![(7, 1_000), (9, 50)],
            ..PendingImportSummary::default()
        };
        assert!(summary.covers_session(7, 1_003)); // runner vs launcher clock skew
        assert!(!summary.covers_session(7, 5_000)); // older leftover of the same game
        assert!(!summary.covers_session(8, 1_000)); // another game
        assert!(!PendingImportSummary::default().covers_session(7, 1_000));
    }
}
