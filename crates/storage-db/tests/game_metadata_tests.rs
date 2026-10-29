//! M2 storage: metadata from scans vs user edits, title override, new filters/sorts,
//! favorites.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use common::{db_with_folder, list_query, play, scanned};
use storage_db::{
    delete_metadata, get_game_detail, get_game_metadata, list_games, list_genres, set_favorite,
    update_user_metadata, upsert_scanned_batch, Connection, GameFilter, GameListQuery,
    GameMetadataRecord, GameSort, ScannedGame, ScannedMetadata, StorageError, UserMetadataEdit,
};

fn with_genre(mut game: ScannedGame, genre: &str) -> ScannedGame {
    game.metadata = Some(ScannedMetadata {
        record: GameMetadataRecord {
            developer: Some("Squaresoft".into()),
            genre: Some(genre.into()),
            release_year: Some(1995),
            players: Some(1),
            ..GameMetadataRecord::default()
        },
        source_version: "v1",
    });
    game
}

/// Chrono (RPG, added t=1), Mario (Platform, added t=2), Zelda (unmatched, added t=3).
fn seeded() -> (Connection, i64) {
    let (mut conn, folder) = db_with_folder();
    let chrono = with_genre(
        scanned(folder, "chrono.sfc", "C1", "Chrono Trigger", true),
        "RPG",
    );
    let mario = with_genre(
        scanned(folder, "mario.sfc", "M1", "Super Mario", true),
        "Platform",
    );
    upsert_scanned_batch(&mut conn, &[chrono], 1).unwrap();
    upsert_scanned_batch(&mut conn, &[mario], 2).unwrap();
    let zelda = scanned(folder, "zelda.sfc", "Z1", "Zelda", false);
    upsert_scanned_batch(&mut conn, &[zelda], 3).unwrap();
    (conn, folder)
}

fn titles(conn: &Connection, query: &GameListQuery) -> Vec<String> {
    list_games(conn, query)
        .unwrap()
        .into_iter()
        .map(|g| g.title)
        .collect()
}

#[test]
fn scan_writes_metadata_and_rescan_keeps_user_edits() {
    let (mut conn, folder) = seeded();
    let chrono = get_game_metadata(&conn, 1).unwrap().unwrap();
    assert_eq!(chrono.genre.as_deref(), Some("RPG"));
    assert_eq!((chrono.release_year, chrono.players), (Some(1995), Some(1)));
    assert!(get_game_metadata(&conn, 3).unwrap().is_none(), "unmatched");

    let edit = UserMetadataEdit {
        genre: Some(" Action RPG ".into()),
        ..UserMetadataEdit::default()
    };
    update_user_metadata(&conn, 1, &edit, "v1", 5).unwrap();
    let rescan = with_genre(
        scanned(folder, "chrono.sfc", "C1", "Chrono Trigger", true),
        "RPG",
    );
    upsert_scanned_batch(&mut conn, &[rescan], 6).unwrap();
    let kept = get_game_metadata(&conn, 1).unwrap().unwrap();
    assert_eq!(kept.genre.as_deref(), Some("Action RPG"));
    assert_eq!(
        kept.developer, None,
        "whole row frozen, blanks saved as none"
    );
    assert!(kept.user_edited);

    delete_metadata(&conn, 1).unwrap();
    assert!(get_game_metadata(&conn, 1).unwrap().is_none());
}

#[test]
fn changed_file_without_index_metadata_drops_the_old_rows_data() {
    let (mut conn, folder) = seeded();
    // Mario re-hashed into a dump the DAT knows but metadat does not: no stale genre.
    let changed = scanned(folder, "mario.sfc", "M2", "Super Mario", true);
    upsert_scanned_batch(&mut conn, &[changed], 7).unwrap();
    assert!(get_game_metadata(&conn, 2).unwrap().is_none());
}

#[test]
fn user_edits_are_validated_and_capped() {
    let (conn, _) = seeded();
    let bad_year = UserMetadataEdit {
        release_year: Some(1850),
        ..UserMetadataEdit::default()
    };
    let err = update_user_metadata(&conn, 1, &bad_year, "v1", 5).unwrap_err();
    assert!(matches!(err, StorageError::InvalidInput(_)));
    let long = UserMetadataEdit {
        title: Some("x".repeat(500)),
        description: Some("d".repeat(5_000)),
        ..UserMetadataEdit::default()
    };
    update_user_metadata(&conn, 3, &long, "v1", 5).unwrap();
    let meta = get_game_metadata(&conn, 3).unwrap().unwrap();
    assert_eq!(meta.title_override.unwrap().chars().count(), 200);
    assert_eq!(meta.description.unwrap().chars().count(), 4_000);
}

#[test]
fn renamed_game_sorts_and_searches_under_its_new_title() {
    let (conn, _) = seeded();
    let rename = UserMetadataEdit {
        title: Some("Aardvark Quest".into()),
        ..UserMetadataEdit::default()
    };
    update_user_metadata(&conn, 3, &rename, "v1", 5).unwrap();
    let by_title = list_query(None, GameSort::Title, GameFilter::All);
    assert_eq!(
        titles(&conn, &by_title),
        ["Aardvark Quest", "Chrono Trigger", "Super Mario"]
    );
    let search = list_query(Some("aardv"), GameSort::Title, GameFilter::All);
    assert_eq!(titles(&conn, &search), ["Aardvark Quest"]);
    assert!(titles(
        &conn,
        &list_query(Some("zelda"), GameSort::Title, GameFilter::All)
    )
    .is_empty());
}

#[test]
fn genre_favorites_played_recently_added_and_limit() {
    let (conn, _) = seeded();
    let rpg = GameListQuery {
        genre: Some("RPG".into()),
        ..GameListQuery::default()
    };
    assert_eq!(titles(&conn, &rpg), ["Chrono Trigger"]);
    assert_eq!(list_genres(&conn).unwrap(), ["Platform", "RPG"]);

    assert!(set_favorite(&conn, 2, true).unwrap());
    assert!(!set_favorite(&conn, 99, true).unwrap());
    let favorites = list_query(None, GameSort::Title, GameFilter::Favorites);
    assert_eq!(titles(&conn, &favorites), ["Super Mario"]);

    let played = list_query(None, GameSort::RecentlyPlayed, GameFilter::Played);
    assert!(
        titles(&conn, &played).is_empty(),
        "never-played games excluded"
    );
    play(&conn, "zelda.sfc", 10, 60);
    assert_eq!(titles(&conn, &played), ["Zelda"]);

    let newest_two = GameListQuery {
        sort: GameSort::RecentlyAdded,
        limit: Some(2),
        ..GameListQuery::default()
    };
    assert_eq!(titles(&conn, &newest_two), ["Zelda", "Super Mario"]);
}

#[test]
fn detail_composes_game_metadata_and_stats() {
    let (conn, _) = seeded();
    play(&conn, "chrono.sfc", 10, 120);
    let detail = get_game_detail(&conn, 1).unwrap().unwrap();
    assert_eq!(detail.game.genre.as_deref(), Some("RPG"));
    assert_eq!(
        detail.metadata.unwrap().developer.as_deref(),
        Some("Squaresoft")
    );
    assert_eq!(detail.stats.session_count, 1);
    assert!(get_game_detail(&conn, 99).unwrap().is_none());
}
