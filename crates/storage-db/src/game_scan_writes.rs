//! Scanner-facing game writes: batched upsert with sha1 relink, missing marks, rescan cache.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, ToSql};

use crate::domain_types::{FileFingerprint, RelinkedGame, ScannedGame, UpsertOutcome};
use crate::game_metadata_repository::UPSERT_FROM_INDEX_SQL;
use crate::storage_error::StorageResult;

/// Shared column assignments; `?1` = rom_path, `?2..?12` = the rest (see [`row_params`]).
const SET_SCANNED_COLUMNS: &str = "platform_id = ?2, folder_id = ?3, file_size = ?4, \
    file_mtime_ns = ?5, crc32 = ?6, sha1 = ?7, title = ?8, title_search = ?9, region = ?10, \
    dat_name = ?11, serial = ?12, missing = 0";

/// Writes one scan chunk in a single transaction (callers send ~50 files per call so UI
/// reads interleave with a long scan). Per file:
/// 1. same `rom_path` exists → update it (also restores detached/missing rows);
/// 2. else a `missing` row of the same platform has the same sha1 (lowest id when
///    several) → relink it to the new path (moved/renamed file keeps its play history;
///    the old path is reported in [`UpsertOutcome::relinked`]);
/// 3. else insert.
///
/// Then writes the game's index metadata (`ScannedGame::metadata`; user-edited rows are
/// kept); a game without index metadata loses its non-edited metadata row.
///
/// Mark vanished paths with [`mark_missing_paths`] *before* upserting so moves relink.
pub fn upsert_scanned_batch(
    conn: &mut Connection,
    games: &[ScannedGame],
    now: i64,
) -> StorageResult<UpsertOutcome> {
    let tx = conn.transaction()?;
    let mut outcome = UpsertOutcome::default();
    {
        let mut update = tx.prepare_cached(&format!(
            "UPDATE games SET {SET_SCANNED_COLUMNS} WHERE rom_path = ?1 RETURNING id"
        ))?;
        let mut relink_candidate = tx.prepare_cached(
            "SELECT id, rom_path FROM games WHERE sha1 = ?1 AND platform_id = ?2 AND missing = 1
             ORDER BY id LIMIT 1",
        )?;
        let mut relink = tx.prepare_cached(&format!(
            "UPDATE games SET rom_path = ?1, {SET_SCANNED_COLUMNS} WHERE id = ?13"
        ))?;
        let mut insert = tx.prepare_cached(
            "INSERT INTO games (rom_path, platform_id, folder_id, file_size, file_mtime_ns,
                crc32, sha1, title, title_search, region, dat_name, serial, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) RETURNING id",
        )?;
        let mut write_metadata = tx.prepare_cached(UPSERT_FROM_INDEX_SQL)?;
        let mut drop_metadata =
            tx.prepare_cached("DELETE FROM game_metadata WHERE game_id = ?1 AND user_edited = 0")?;
        let id_of = |r: &rusqlite::Row<'_>| r.get::<_, i64>(0);
        for game in games {
            let title_search = title_search_key(&game.title);
            let row = row_params(game, &title_search);
            let game_id = if let Some(id) = update.query_row(&row[..], id_of).optional()? {
                outcome.updated += 1;
                id
            } else if let Some((id, old_rom_path)) = relink_candidate
                .query_row(params![game.sha1, game.platform_id], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })
                .optional()?
            {
                let mut with_id: Vec<&dyn ToSql> = row.to_vec();
                with_id.push(&id);
                relink.execute(&with_id[..])?;
                outcome.relinked.push(RelinkedGame {
                    game_id: id,
                    platform_id: game.platform_id.clone(),
                    old_rom_path,
                    new_rom_path: game.rom_path.clone(),
                });
                id
            } else {
                let mut with_added_at: Vec<&dyn ToSql> = row.to_vec();
                with_added_at.push(&now);
                outcome.inserted += 1;
                insert.query_row(&with_added_at[..], id_of)?
            };
            if let Some(meta) = &game.metadata {
                let r = &meta.record;
                write_metadata.execute(params![
                    game_id,
                    r.developer,
                    r.publisher,
                    r.genre,
                    r.release_year,
                    r.players,
                    meta.source_version,
                    now
                ])?;
            } else {
                // Unidentified, or not in metadat: the old file's index data no longer
                // applies (a later full index load re-heals matched games).
                drop_metadata.execute(params![game_id])?;
            }
        }
    }
    tx.commit()?;
    Ok(outcome)
}

/// Positional parameters `?1..?12` matching [`SET_SCANNED_COLUMNS`] and the insert.
fn row_params<'a>(game: &'a ScannedGame, title_search: &'a dyn ToSql) -> [&'a dyn ToSql; 12] {
    [
        &game.rom_path,
        &game.platform_id,
        &game.folder_id,
        &game.file_size,
        &game.file_mtime_ns,
        &game.crc32,
        &game.sha1,
        &game.title,
        title_search,
        &game.region,
        &game.dat_name,
        &game.serial,
    ]
}

/// Search key: Unicode lower case (SQLite `LIKE` only folds ASCII).
pub(crate) fn title_search_key(title: &str) -> String {
    title.to_lowercase()
}

/// Flags files that vanished from disk; returns rows changed. One transaction.
pub fn mark_missing_paths(conn: &mut Connection, rom_paths: &[String]) -> StorageResult<usize> {
    let tx = conn.transaction()?;
    let mut changed = 0;
    {
        let mut stmt =
            tx.prepare_cached("UPDATE games SET missing = 1 WHERE rom_path = ?1 AND missing = 0")?;
        for path in rom_paths {
            changed += stmt.execute(params![path])?;
        }
    }
    tx.commit()?;
    Ok(changed)
}

/// Rescan cache for one folder: present (not missing) files and their last fingerprint.
/// Detached or missing rows are left out on purpose so a returning file goes through
/// [`upsert_scanned_batch`], which restores it.
pub fn file_fingerprints(
    conn: &Connection,
    folder_id: i64,
) -> StorageResult<HashMap<String, FileFingerprint>> {
    let mut stmt = conn.prepare_cached(
        "SELECT rom_path, file_size, file_mtime_ns FROM games WHERE folder_id = ?1 AND missing = 0",
    )?;
    let rows = stmt.query_map(params![folder_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            FileFingerprint {
                size: r.get(1)?,
                mtime_ns: r.get(2)?,
            },
        ))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
