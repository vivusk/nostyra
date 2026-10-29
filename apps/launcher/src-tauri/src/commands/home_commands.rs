//! Home page data in one call: three small `list_games` queries under one lock.

use serde::Serialize;
use storage_db::{Connection, Game, GameFilter, GameListQuery, GameSort, StorageResult};
use tauri::State;

use crate::app_state::AppState;
use crate::asset_scope_dirs::with_absolute_art;
use crate::command_error::CommandResult;

const CONTINUE_LIMIT: u32 = 4;
const RECENTLY_ADDED_LIMIT: u32 = 6;
/// A few favorites so the hero selector can pick one not already listed.
const FAVORITES_LIMIT: u32 = 4;

/// Played games (most recent first), newest added games, favorites. The UI builds the
/// hero selector from these (last played ×2, newest added, a favorite).
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeData {
    pub continue_playing: Vec<Game>,
    pub recently_added: Vec<Game>,
    pub favorites: Vec<Game>,
}

fn query(sort: GameSort, filter: GameFilter, limit: u32) -> GameListQuery {
    GameListQuery {
        sort,
        filter,
        limit: Some(limit),
        ..GameListQuery::default()
    }
}

/// The three lists with DB-relative art paths (callers make them absolute).
pub fn home_lists(conn: &Connection) -> StorageResult<HomeData> {
    Ok(HomeData {
        // `Played` = played at least once: never-played games never fill this row.
        continue_playing: storage_db::list_games(
            conn,
            &query(GameSort::RecentlyPlayed, GameFilter::Played, CONTINUE_LIMIT),
        )?,
        recently_added: storage_db::list_games(
            conn,
            &query(
                GameSort::RecentlyAdded,
                GameFilter::All,
                RECENTLY_ADDED_LIMIT,
            ),
        )?,
        favorites: storage_db::list_games(
            conn,
            &query(
                GameSort::RecentlyPlayed,
                GameFilter::Favorites,
                FAVORITES_LIMIT,
            ),
        )?,
    })
}

#[tauri::command]
pub fn get_home_data(state: State<'_, AppState>) -> CommandResult<HomeData> {
    let lists = home_lists(&state.db())?;
    let dir = &state.paths.artwork_dir;
    let absolute = |games: Vec<Game>| {
        games
            .into_iter()
            .map(|g| with_absolute_art(g, dir))
            .collect()
    };
    Ok(HomeData {
        continue_playing: absolute(lists.continue_playing),
        recently_added: absolute(lists.recently_added),
        favorites: absolute(lists.favorites),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage_db::{
        add_library_folder, open_in_memory_database, upsert_scanned_batch, ScannedGame,
    };

    #[test]
    fn unplayed_library_has_no_continue_row() {
        let mut conn = open_in_memory_database().unwrap();
        let folder = add_library_folder(&conn, "roms", 1).unwrap().id;
        let games: Vec<ScannedGame> = (0..8)
            .map(|i| ScannedGame {
                platform_id: "snes".into(),
                folder_id: folder,
                rom_path: format!("roms/{i}.sfc"),
                file_size: 1,
                file_mtime_ns: 0,
                crc32: format!("{i:08X}"),
                sha1: format!("S{i}"),
                title: format!("Game {i}"),
                region: None,
                dat_name: None,
                metadata: None,
                serial: None,
            })
            .collect();
        upsert_scanned_batch(&mut conn, &games, 1).unwrap();
        let home = home_lists(&conn).unwrap();
        assert!(home.continue_playing.is_empty());
        assert_eq!(home.recently_added.len(), 6);
        assert!(home.favorites.is_empty());
    }
}
