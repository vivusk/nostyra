//! Artwork lookup results: one row per (game, kind) for cached / not-found / failed.
//! Transient failures write nothing, so they are simply retried by the next job.

use rusqlite::{named_params, params, Connection};

use crate::metadata_and_artwork_types::{ArtworkRecord, ArtworkStatus, ArtworkWorkItem};
use crate::storage_error::StorageResult;

/// All artwork rows of a game.
pub fn get_artwork(conn: &Connection, game_id: i64) -> StorageResult<Vec<ArtworkRecord>> {
    let mut stmt = conn.prepare_cached(
        "SELECT game_id, kind, status, local_path, source, source_name, checked_at
         FROM artwork WHERE game_id = ?1 ORDER BY kind",
    )?;
    let rows = stmt.query_map(params![game_id], |r| {
        Ok(ArtworkRecord {
            game_id: r.get(0)?,
            kind: r.get(1)?,
            status: ArtworkStatus::parse(&r.get::<_, String>(2)?),
            local_path: r.get(3)?,
            source: r.get(4)?,
            source_name: r.get(5)?,
            checked_at: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Inserts or replaces the row of `(game_id, kind)`.
pub fn upsert_artwork(conn: &Connection, record: &ArtworkRecord) -> StorageResult<()> {
    conn.prepare_cached(
        "INSERT INTO artwork (game_id, kind, status, local_path, source, source_name, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(game_id, kind) DO UPDATE SET status = excluded.status,
            local_path = excluded.local_path, source = excluded.source,
            source_name = excluded.source_name, checked_at = excluded.checked_at",
    )?
    .execute(params![
        record.game_id,
        record.kind,
        record.status.as_str(),
        record.local_path,
        record.source,
        record.source_name,
        record.checked_at
    ])?;
    Ok(())
}

/// Games to fetch `kind` for: No-Intro-matched, in a library folder, present on disk, and
/// either never looked up or a non-cached result older than `retry_before`. Oldest first.
pub fn games_needing_artwork(
    conn: &Connection,
    kind: &str,
    retry_before: i64,
    limit: u32,
) -> StorageResult<Vec<ArtworkWorkItem>> {
    let mut stmt = conn.prepare_cached(
        "SELECT g.id, g.platform_id, g.dat_name FROM games g
         LEFT JOIN artwork a ON a.game_id = g.id AND a.kind = :kind
         WHERE g.dat_name IS NOT NULL AND g.folder_id IS NOT NULL AND g.missing = 0
           AND (a.game_id IS NULL OR (a.status <> 'cached' AND a.checked_at < :retry_before))
         ORDER BY g.id LIMIT :limit",
    )?;
    let rows = stmt.query_map(
        named_params! { ":kind": kind, ":retry_before": retry_before, ":limit": limit },
        |r| {
            Ok(ArtworkWorkItem {
                game_id: r.get(0)?,
                platform_id: r.get(1)?,
                dat_name: r.get(2)?,
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Kinds among `kinds` that one game has no cached file for (user-forced fetch: the
/// retry window is ignored).
pub fn uncached_artwork_kinds<'k>(
    conn: &Connection,
    game_id: i64,
    kinds: &[&'k str],
) -> StorageResult<Vec<&'k str>> {
    let mut stmt = conn.prepare_cached(
        "SELECT COUNT(*) FROM artwork WHERE game_id = ?1 AND kind = ?2 AND status = 'cached'",
    )?;
    let mut missing = Vec::new();
    for &kind in kinds {
        let cached: i64 = stmt.query_row(params![game_id, kind], |r| r.get(0))?;
        if cached == 0 {
            missing.push(kind);
        }
    }
    Ok(missing)
}

/// Deletes every artwork row (Clear cache; the caller removes the files). Returns rows deleted.
pub fn clear_artwork(conn: &Connection) -> StorageResult<usize> {
    Ok(conn.execute("DELETE FROM artwork", [])?)
}
