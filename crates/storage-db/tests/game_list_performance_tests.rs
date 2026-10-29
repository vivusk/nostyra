//! Plan success criterion: listing 5 000 games (with metadata + artwork joins) stays
//! under 20 ms.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::time::Instant;

use common::{db_with_folder, list_query, now, play, scanned};
use storage_db::{
    list_games, upsert_artwork, upsert_scanned_batch, ArtworkRecord, ArtworkStatus, GameFilter,
    GameListQuery, GameMetadataRecord, GameSort, ScannedGame, ScannedMetadata,
};

#[test]
fn five_thousand_game_list_is_fast() {
    let (mut conn, folder) = db_with_folder();
    let games: Vec<ScannedGame> = (0..5_000)
        .map(|i| {
            let mut game = scanned(
                folder,
                &format!("g{i}.sfc"),
                &format!("S{i}"),
                &format!("Game {i}"),
                i % 7 != 0,
            );
            game.metadata = game.dat_name.is_some().then(|| ScannedMetadata {
                record: GameMetadataRecord {
                    genre: Some(format!("Genre {}", i % 12)),
                    release_year: Some(1990 + (i % 10) as u16),
                    ..GameMetadataRecord::default()
                },
                source_version: "v1",
            });
            game
        })
        .collect();
    for chunk in games.chunks(50) {
        upsert_scanned_batch(&mut conn, chunk, 1).unwrap();
    }
    for i in (0..5_000).step_by(3) {
        play(&conn, &format!("g{i}.sfc"), now() - i, 60);
    }
    for game_id in (1..=5_000).step_by(2) {
        let record = ArtworkRecord {
            game_id,
            kind: "cover".into(),
            status: ArtworkStatus::Cached,
            local_path: Some(format!("{game_id}-cover.jpg")),
            source: "libretro-thumbnails".into(),
            source_name: None,
            checked_at: 1,
        };
        upsert_artwork(&conn, &record).unwrap();
        let hero = ArtworkRecord {
            kind: "hero".into(),
            local_path: Some(format!("{game_id}-hero.png")),
            ..record
        };
        upsert_artwork(&conn, &hero).unwrap();
    }
    let by_genre = GameListQuery {
        genre: Some("Genre 3".into()),
        ..GameListQuery::default()
    };
    for (sort, search, genre) in [
        (GameSort::RecentlyPlayed, None, None),
        (GameSort::Title, Some("game 4"), None),
        (GameSort::PlayTime, None, None),
        (GameSort::RecentlyAdded, None, by_genre.genre.clone()),
    ] {
        let q = GameListQuery {
            genre,
            ..list_query(search, sort, GameFilter::All)
        };
        list_games(&conn, &q).unwrap(); // warm the statement cache
        let start = Instant::now();
        let rows = list_games(&conn, &q).unwrap();
        let elapsed = start.elapsed();
        eprintln!(
            "list {sort:?} search={search:?}: {} rows in {elapsed:?}",
            rows.len()
        );
        // Plan target is 20 ms; unoptimized test code gets headroom (SQLite itself is -O2).
        let limit_ms = if cfg!(debug_assertions) { 60 } else { 20 };
        assert!(elapsed.as_millis() < limit_ms, "{sort:?} took {elapsed:?}");
    }
}
