//! Library reads: filtered/sorted game list, header summary, single-game lookups.

use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};

use crate::domain_types::{Game, GameFilter, GameListQuery, LibrarySummary};
use crate::game_list_sql::{list_games_sql, GAME_SELECT};
use crate::game_scan_writes::title_search_key;
use crate::storage_error::StorageResult;

/// "Recently played" filter window: 30 days.
pub const RECENTLY_PLAYED_WINDOW_S: i64 = 30 * 24 * 60 * 60;

/// Visible library games (rows of removed folders are hidden), filtered and sorted.
pub fn list_games(conn: &Connection, query: &GameListQuery) -> StorageResult<Vec<Game>> {
    let sql = list_games_sql(query.filter, query.sort);
    let pattern = search_pattern(query.search.as_deref());
    let genre = query
        .genre
        .as_deref()
        .map(str::trim)
        .filter(|genre| !genre.is_empty());
    let mut named: Vec<(&str, &dyn ToSql)> = vec![
        (":search", &pattern),
        (":genre", &genre),
        (":platform", &query.platform),
        (":limit", &query.limit),
    ];
    // rusqlite rejects names missing from the statement: bind the window only when used.
    if query.filter == GameFilter::RecentlyPlayed {
        named.push((":window", &RECENTLY_PLAYED_WINDOW_S));
    }
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt
        .query_map(&named[..], game_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// `LIKE` pattern for a search box value: lower-cased, wildcards escaped; blank → `None`.
fn search_pattern(search: Option<&str>) -> Option<String> {
    let trimmed = search?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut pattern = String::from("%");
    for ch in title_search_key(trimmed).chars() {
        if matches!(ch, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(ch);
    }
    pattern.push('%');
    Some(pattern)
}

/// Counts for the library header (visible games only).
pub fn library_summary(conn: &Connection) -> StorageResult<LibrarySummary> {
    Ok(conn.query_row(
        "SELECT COUNT(*), COUNT(dat_name), COUNT(*) - COUNT(dat_name), COALESCE(SUM(missing), 0)
         FROM games WHERE folder_id IS NOT NULL",
        [],
        |r| {
            Ok(LibrarySummary {
                total: r.get(0)?,
                matched: r.get(1)?,
                needs_review: r.get(2)?,
                missing: r.get(3)?,
            })
        },
    )?)
}

/// One game by id (hidden rows included: Play needs it even while history is shown).
pub fn get_game(conn: &Connection, game_id: i64) -> StorageResult<Option<Game>> {
    let sql = format!("{GAME_SELECT} WHERE g.id = ?1");
    Ok(conn
        .query_row(&sql, params![game_id], game_from_row)
        .optional()?)
}

/// Game id for a ROM path (pending-session import matches sessions by path).
pub fn find_game_id_by_rom_path(conn: &Connection, rom_path: &str) -> StorageResult<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM games WHERE rom_path = ?1",
            params![rom_path],
            |r| r.get(0),
        )
        .optional()?)
}

/// Visible games per platform id (Library system filter), platform id order.
pub fn library_platform_counts(conn: &Connection) -> StorageResult<Vec<(String, i64)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT platform_id, COUNT(*) FROM games WHERE folder_id IS NOT NULL
         GROUP BY platform_id ORDER BY platform_id",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Platform ids that have at least one game row (startup catalog warm-up).
pub fn game_platform_ids(conn: &Connection) -> StorageResult<Vec<String>> {
    let mut stmt = conn.prepare_cached("SELECT DISTINCT platform_id FROM games ORDER BY 1")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub(crate) fn game_from_row(r: &Row<'_>) -> rusqlite::Result<Game> {
    Ok(Game {
        id: r.get(0)?,
        platform_id: r.get(1)?,
        folder_id: r.get(2)?,
        rom_path: r.get(3)?,
        file_size: r.get(4)?,
        crc32: r.get(5)?,
        sha1: r.get(6)?,
        title: r.get(7)?,
        region: r.get(8)?,
        dat_name: r.get(9)?,
        added_at: r.get(10)?,
        missing: r.get(11)?,
        total_play_s: r.get(12)?,
        last_played_at: r.get(13)?,
        genre: r.get(14)?,
        release_year: r.get(15)?,
        developer: r.get(16)?,
        favorite: r.get(17)?,
        cover_path: r.get(18)?,
        hero_path: r.get(19)?,
        cover_checked_at: r.get(20)?,
        serial: r.get(21)?,
    })
}

#[cfg(test)]
mod tests {
    use super::search_pattern;

    #[test]
    fn search_pattern_escapes_wildcards_and_lowercases() {
        assert_eq!(search_pattern(None), None);
        assert_eq!(search_pattern(Some("  ")), None);
        assert_eq!(
            search_pattern(Some("Pokémon")).as_deref(),
            Some("%pokémon%")
        );
        assert_eq!(
            search_pattern(Some("100%_a")).as_deref(),
            Some("%100\\%\\_a%")
        );
    }
}
