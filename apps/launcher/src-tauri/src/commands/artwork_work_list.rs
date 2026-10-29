//! Builds the games an artwork pass fetches: all missing (respecting the 7-day retry
//! window) or user-picked games (window ignored: a forced retry).

use std::collections::BTreeMap;

use artwork_cache::{ArtworkKind, GameArtRequest};
use storage_db::{ArtworkWorkItem, Connection, StorageResult};

/// Not-found / failed lookups are retried after this long.
pub const ARTWORK_RETRY_S: i64 = 7 * 24 * 60 * 60;
/// Upper bound per pass; a bigger library is finished by the next pass.
const WORK_LIMIT: u32 = 20_000;
const KINDS: [ArtworkKind; 2] = [ArtworkKind::Cover, ArtworkKind::Hero];

/// Every game missing at least one kind; one request per game with its missing kinds.
pub fn missing_artwork_requests(conn: &Connection, now: i64) -> StorageResult<Vec<GameArtRequest>> {
    let retry_before = now - ARTWORK_RETRY_S;
    let mut per_kind = Vec::new();
    for kind in KINDS {
        let items =
            storage_db::games_needing_artwork(conn, kind.as_str(), retry_before, WORK_LIMIT)?;
        per_kind.push((kind, items));
    }
    Ok(merge_by_game(per_kind))
}

/// Requests for specific games: kinds without a cached file, retry window ignored.
/// Games without a No-Intro name (needs review) have nothing to look up and are skipped.
pub fn game_artwork_requests(
    conn: &Connection,
    game_ids: &[i64],
) -> StorageResult<Vec<GameArtRequest>> {
    let mut requests = Vec::new();
    for &game_id in game_ids {
        let Some(game) = storage_db::get_game(conn, game_id)? else {
            continue;
        };
        let (Some(dat_name), Some(system)) =
            (game.dat_name, system_catalog::system(&game.platform_id))
        else {
            continue;
        };
        let names = KINDS.map(ArtworkKind::as_str);
        let missing = storage_db::uncached_artwork_kinds(conn, game_id, &names)?;
        let kinds: Vec<ArtworkKind> = KINDS
            .into_iter()
            .filter(|kind| missing.contains(&kind.as_str()))
            .collect();
        if !kinds.is_empty()
            && !requests
                .iter()
                .any(|r: &GameArtRequest| r.game_id == game_id)
        {
            requests.push(GameArtRequest {
                game_id,
                system,
                no_intro_name: dat_name,
                kinds,
            });
        }
    }
    Ok(requests)
}

/// Joins per-kind work lists into one request per game (ordered by id). Games of a
/// platform outside the registry have no thumbnail repo and are skipped.
fn merge_by_game(per_kind: Vec<(ArtworkKind, Vec<ArtworkWorkItem>)>) -> Vec<GameArtRequest> {
    let mut by_game: BTreeMap<i64, GameArtRequest> = BTreeMap::new();
    for (kind, items) in per_kind {
        for item in items {
            let Some(system) = system_catalog::system(&item.platform_id) else {
                continue;
            };
            by_game
                .entry(item.game_id)
                .or_insert_with(|| GameArtRequest {
                    game_id: item.game_id,
                    system,
                    no_intro_name: item.dat_name,
                    kinds: Vec::new(),
                })
                .kinds
                .push(kind);
        }
    }
    by_game.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage_db::{
        add_library_folder, open_in_memory_database, upsert_artwork, upsert_scanned_batch,
        ArtworkRecord, ArtworkStatus, ScannedGame,
    };

    fn item(game_id: i64) -> ArtworkWorkItem {
        ArtworkWorkItem {
            game_id,
            platform_id: "snes".into(),
            dat_name: format!("Game {game_id} (USA)"),
        }
    }

    #[test]
    fn merges_kinds_per_game_in_id_order() {
        let merged = merge_by_game(vec![
            (ArtworkKind::Cover, vec![item(3), item(1)]),
            (ArtworkKind::Hero, vec![item(1), item(2)]),
        ]);
        let shape: Vec<(i64, Vec<ArtworkKind>)> =
            merged.into_iter().map(|r| (r.game_id, r.kinds)).collect();
        assert_eq!(
            shape,
            [
                (1, vec![ArtworkKind::Cover, ArtworkKind::Hero]),
                (2, vec![ArtworkKind::Hero]),
                (3, vec![ArtworkKind::Cover]),
            ]
        );
    }

    /// One matched game (id 1) and one needs-review game (id 2).
    fn library() -> Connection {
        let mut conn = open_in_memory_database().unwrap();
        let folder = add_library_folder(&conn, "roms", 1).unwrap().id;
        let games: Vec<ScannedGame> = [Some("Chrono Trigger (USA)"), None]
            .into_iter()
            .enumerate()
            .map(|(i, dat)| ScannedGame {
                platform_id: "snes".into(),
                folder_id: folder,
                rom_path: format!("roms/{i}.sfc"),
                file_size: 1,
                file_mtime_ns: 0,
                crc32: format!("{i:08X}"),
                sha1: format!("S{i}"),
                title: format!("Game {i}"),
                region: None,
                dat_name: dat.map(str::to_string),
                metadata: None,
                serial: None,
            })
            .collect();
        upsert_scanned_batch(&mut conn, &games, 1).unwrap();
        conn
    }

    fn row(kind: &str, status: ArtworkStatus, checked_at: i64) -> ArtworkRecord {
        ArtworkRecord {
            game_id: 1,
            kind: kind.into(),
            status,
            local_path: None,
            source: "libretro-thumbnails".into(),
            source_name: None,
            checked_at,
        }
    }

    #[test]
    fn games_without_rows_are_selected_again_and_the_retry_window_holds() {
        let conn = library();
        // A transient run wrote nothing: the game is simply selected again.
        let first = missing_artwork_requests(&conn, 1_000_000).unwrap();
        assert_eq!(first.len(), 1, "needs-review game skipped");
        assert_eq!(first[0].kinds, [ArtworkKind::Cover, ArtworkKind::Hero]);

        upsert_artwork(&conn, &row("cover", ArtworkStatus::NotFound, 1_000_000)).unwrap();
        let next = missing_artwork_requests(&conn, 1_000_000).unwrap();
        assert_eq!(next[0].kinds, [ArtworkKind::Hero], "not-found waits 7 days");
        // A per-game request ignores the window.
        let forced = game_artwork_requests(&conn, &[1, 2, 99]).unwrap();
        assert_eq!(forced.len(), 1);
        assert_eq!(forced[0].kinds, [ArtworkKind::Cover, ArtworkKind::Hero]);
    }
}
