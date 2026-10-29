//! Play sessions and per-game play stats.

use rusqlite::{params, Connection};

use crate::domain_types::{NewPlaySession, PlayStats};
use crate::storage_error::StorageResult;

/// Records a session. A repeated `source_key` is ignored (pending-file import is
/// idempotent); returns whether a row was inserted.
pub fn record_play_session(conn: &Connection, session: &NewPlaySession) -> StorageResult<bool> {
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO play_sessions (game_id, started_at, duration_s, exit_code, source_key)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            session.game_id,
            session.started_at,
            session.duration_s.max(0),
            session.exit_code,
            session.source_key
        ],
    )?;
    Ok(inserted > 0)
}

/// Total play time, last session start and session count; zeros for a never-played game.
pub fn game_play_stats(conn: &Connection, game_id: i64) -> StorageResult<PlayStats> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(duration_s), 0), MAX(started_at), COUNT(*)
         FROM play_sessions WHERE game_id = ?1",
        params![game_id],
        |r| {
            Ok(PlayStats {
                total_play_s: r.get(0)?,
                last_played_at: r.get(1)?,
                session_count: r.get(2)?,
            })
        },
    )?)
}
