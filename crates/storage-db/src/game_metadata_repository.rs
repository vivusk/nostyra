//! Game metadata (index-derived or user-edited), favorites and the composed detail.

use rusqlite::{named_params, params, Connection, OptionalExtension};

use crate::game_queries::get_game;
use crate::metadata_and_artwork_types::{GameDetail, GameMetadata, GameMetadataRecord};
use crate::play_session_repository::game_play_stats;
use crate::storage_error::StorageResult;

/// Writes index fields unless the row is user-edited (edits survive rescans/self-heal).
/// Params: `?1` game id, `?2..?6` developer, publisher, genre, year, players,
/// `?7` source version, `?8` now.
pub(crate) const UPSERT_FROM_INDEX_SQL: &str = "INSERT INTO game_metadata
        (game_id, developer, publisher, genre, release_year, players, source_version, updated_at)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
    ON CONFLICT(game_id) DO UPDATE SET developer = excluded.developer,
        publisher = excluded.publisher, genre = excluded.genre,
        release_year = excluded.release_year, players = excluded.players,
        source_version = excluded.source_version, updated_at = excluded.updated_at
    WHERE game_metadata.user_edited = 0";

/// Applies index metadata to one game (no-op on a user-edited row).
pub fn upsert_from_index(
    conn: &Connection,
    game_id: i64,
    record: &GameMetadataRecord,
    source_version: &str,
    now: i64,
) -> StorageResult<()> {
    conn.prepare_cached(UPSERT_FROM_INDEX_SQL)?
        .execute(params![
            game_id,
            record.developer,
            record.publisher,
            record.genre,
            record.release_year,
            record.players,
            source_version,
            now
        ])?;
    Ok(())
}

/// Removes the metadata row (first half of "reset to database values").
pub fn delete_metadata(conn: &Connection, game_id: i64) -> StorageResult<()> {
    conn.execute(
        "DELETE FROM game_metadata WHERE game_id = ?1",
        params![game_id],
    )?;
    Ok(())
}

/// The stored metadata row of a game, if any.
pub fn get_game_metadata(conn: &Connection, game_id: i64) -> StorageResult<Option<GameMetadata>> {
    Ok(conn
        .query_row(
            "SELECT title_override, developer, publisher, genre, release_year, players,
                description, user_edited, updated_at
             FROM game_metadata WHERE game_id = ?1",
            params![game_id],
            |r| {
                Ok(GameMetadata {
                    title_override: r.get(0)?,
                    developer: r.get(1)?,
                    publisher: r.get(2)?,
                    genre: r.get(3)?,
                    release_year: r.get(4)?,
                    players: r.get(5)?,
                    description: r.get(6)?,
                    user_edited: r.get(7)?,
                    updated_at: r.get(8)?,
                })
            },
        )
        .optional()?)
}

/// Game + metadata + play stats for the Detail page.
pub fn get_game_detail(conn: &Connection, game_id: i64) -> StorageResult<Option<GameDetail>> {
    let Some(game) = get_game(conn, game_id)? else {
        return Ok(None);
    };
    Ok(Some(GameDetail {
        metadata: get_game_metadata(conn, game_id)?,
        stats: game_play_stats(conn, game_id)?,
        game,
    }))
}

/// Distinct genres of visible games, A–Z (Library genre filter).
pub fn list_genres(conn: &Connection) -> StorageResult<Vec<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT m.genre FROM game_metadata m JOIN games g ON g.id = m.game_id
         WHERE g.folder_id IS NOT NULL AND m.genre IS NOT NULL ORDER BY m.genre",
    )?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// One self-heal work item: enough to find the game's metadat record in its system's
/// catalog (`dat_name` → metadata CRC, else the stored `crc32`; discs: `serial`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataWorkItem {
    pub game_id: i64,
    pub platform_id: String,
    pub crc32: String,
    pub dat_name: Option<String>,
    pub serial: Option<String>,
}

/// Self-heal work list: DAT-matched games without a metadata row and, when `stale`,
/// non-edited rows written by another metadat version.
pub fn games_needing_metadata(
    conn: &Connection,
    version: &str,
    stale: bool,
) -> StorageResult<Vec<MetadataWorkItem>> {
    let mut stmt = conn.prepare_cached(
        "SELECT g.id, g.platform_id, g.crc32, g.dat_name, g.serial
         FROM games g LEFT JOIN game_metadata m ON m.game_id = g.id
         WHERE g.dat_name IS NOT NULL AND (m.game_id IS NULL
            OR (:stale AND m.user_edited = 0 AND m.source_version <> :version))",
    )?;
    let rows = stmt.query_map(
        named_params! { ":stale": stale, ":version": version },
        |r| {
            Ok(MetadataWorkItem {
                game_id: r.get(0)?,
                platform_id: r.get(1)?,
                crc32: r.get(2)?,
                dat_name: r.get(3)?,
                serial: r.get(4)?,
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Sets or clears the favorite flag; returns whether the game exists.
pub fn set_favorite(conn: &Connection, game_id: i64, favorite: bool) -> StorageResult<bool> {
    let changed = conn.execute(
        "UPDATE games SET favorite = ?2 WHERE id = ?1",
        params![game_id, favorite],
    )?;
    Ok(changed > 0)
}
