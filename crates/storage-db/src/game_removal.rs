//! Removing missing games for good: the row and everything that cascades from it
//! (play sessions, metadata, artwork rows). The launcher deletes the game's files
//! (saves, game config, cached art) before calling [`delete_missing_game`].

use rusqlite::{params, Connection};

use crate::domain_types::GameFilesKey;
use crate::storage_error::StorageResult;

/// Ids of the visible games whose ROM file is missing (the Library "Missing" list).
/// Rows of removed folders are hidden and kept for a re-add, so they are left out.
pub fn visible_missing_game_ids(conn: &Connection) -> StorageResult<Vec<i64>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id FROM games WHERE missing = 1 AND folder_id IS NOT NULL ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// What names every game's own files (save folder, game config layer), optionally
/// for one platform only: the launcher keeps files another row still resolves to
/// and matches legacy folders to games.
pub fn game_files_keys(
    conn: &Connection,
    platform_id: Option<&str>,
) -> StorageResult<Vec<GameFilesKey>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, platform_id, rom_path, sha1 FROM games
         WHERE ?1 IS NULL OR platform_id = ?1 ORDER BY id",
    )?;
    let rows = stmt.query_map(params![platform_id], |r| {
        Ok(GameFilesKey {
            game_id: r.get(0)?,
            platform_id: r.get(1)?,
            rom_path: r.get(2)?,
            sha1: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Deletes a game whose ROM is missing (cascades to its sessions, metadata and
/// artwork rows). Returns `false` when the id is unknown or the ROM is present
/// again (a rescan found it), so a present game is never removed.
pub fn delete_missing_game(conn: &Connection, game_id: i64) -> StorageResult<bool> {
    let deleted = conn.execute(
        "DELETE FROM games WHERE id = ?1 AND missing = 1",
        params![game_id],
    )?;
    Ok(deleted > 0)
}
