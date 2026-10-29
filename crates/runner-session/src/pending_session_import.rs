//! Imports play-session files the runner left in `<data>/sessions/pending/` into
//! `play_sessions`, then removes them. Runs after each session and at launcher
//! start, so play time survives the launcher closing mid-game. Idempotent: the file
//! name is the row's `source_key` (`INSERT OR IGNORE`), and a file is removed only
//! after its row is committed.

use std::path::Path;

use ipc_protocol::{pending_sessions_dir, PendingSessionRecord};
use storage_db::{find_game_id_by_rom_path, record_play_session, Connection, NewPlaySession};

/// Runner Started vs launcher receipt can differ by process start latency.
const SAME_SESSION_TOLERANCE_S: i64 = 120;

/// What an import pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingImportSummary {
    pub imported: usize,
    /// Already recorded earlier (file left behind by an interrupted import).
    pub duplicates: usize,
    /// ROM path not in the library (game removed): dropped.
    pub unknown_game: usize,
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

/// Imports every `*.json` pending session under `data_root`. Missing folder = nothing to do.
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
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        // `*.json.tmp` = a write in progress (or interrupted): not ours to touch.
        if !file_name.ends_with(".json") {
            continue;
        }
        let record = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<PendingSessionRecord>(&bytes).ok());
        match record {
            None => {
                tracing::warn!(file = %path.display(), "dropping unreadable pending session");
                summary.invalid += 1;
            }
            Some(record) => match find_game_id_by_rom_path(conn, &record.rom_path)? {
                None => {
                    tracing::info!(rom = %record.rom_path, "pending session for unknown game dropped");
                    summary.unknown_game += 1;
                }
                Some(game_id) => {
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
                }
            },
        }
        if let Err(err) = std::fs::remove_file(&path) {
            // Harmless: the next pass sees a duplicate and retries the removal.
            tracing::warn!(file = %path.display(), %err, "cannot remove pending session");
        }
    }
    Ok(summary)
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
