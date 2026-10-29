//! Artwork rows: work list (retry window, matched games only), per-game uncached kinds,
//! list paths, clear.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use common::{db_with_folder, list_query, scanned};
use storage_db::{
    clear_artwork, games_needing_artwork, list_games, uncached_artwork_kinds, upsert_artwork,
    upsert_scanned_batch, ArtworkRecord, ArtworkStatus, ArtworkWorkItem, Connection, GameFilter,
    GameSort,
};

/// Chrono + Mario (matched), Zelda (unmatched).
fn seeded() -> Connection {
    let (mut conn, folder) = db_with_folder();
    let games = [
        scanned(folder, "chrono.sfc", "C1", "Chrono Trigger", true),
        scanned(folder, "mario.sfc", "M1", "Super Mario", true),
        scanned(folder, "zelda.sfc", "Z1", "Zelda", false),
    ];
    upsert_scanned_batch(&mut conn, &games, 1).unwrap();
    conn
}

fn art(game_id: i64, kind: &str, status: ArtworkStatus, checked_at: i64) -> ArtworkRecord {
    ArtworkRecord {
        game_id,
        kind: kind.into(),
        status,
        local_path: (status == ArtworkStatus::Cached).then(|| format!("{game_id}-{kind}.jpg")),
        source: "libretro-thumbnails".into(),
        source_name: None,
        checked_at,
    }
}

#[test]
fn artwork_work_list_and_list_paths() {
    let conn = seeded();
    let ids =
        |list: Vec<ArtworkWorkItem>| -> Vec<i64> { list.into_iter().map(|w| w.game_id).collect() };
    // Matched games only (Zelda has no No-Intro name).
    assert_eq!(
        ids(games_needing_artwork(&conn, "cover", 0, 10).unwrap()),
        [1, 2]
    );

    upsert_artwork(&conn, &art(1, "cover", ArtworkStatus::Cached, 100)).unwrap();
    upsert_artwork(&conn, &art(2, "cover", ArtworkStatus::NotFound, 100)).unwrap();
    assert!(games_needing_artwork(&conn, "cover", 100, 10)
        .unwrap()
        .is_empty());
    // Past the retry window the not-found game comes back; the cached one never does.
    assert_eq!(
        ids(games_needing_artwork(&conn, "cover", 101, 10).unwrap()),
        [2]
    );
    assert_eq!(
        uncached_artwork_kinds(&conn, 2, &["cover", "hero"]).unwrap(),
        ["cover", "hero"]
    );
    assert_eq!(
        uncached_artwork_kinds(&conn, 1, &["cover", "hero"]).unwrap(),
        ["hero"]
    );

    let games = list_games(&conn, &list_query(None, GameSort::Title, GameFilter::All)).unwrap();
    assert_eq!(games[0].cover_path.as_deref(), Some("1-cover.jpg"));
    assert_eq!(games[0].cover_checked_at, Some(100));
    assert_eq!(games[1].cover_path, None, "not-found rows expose no path");

    assert_eq!(clear_artwork(&conn).unwrap(), 2);
    assert_eq!(
        games_needing_artwork(&conn, "cover", 0, 10).unwrap().len(),
        2
    );
}
