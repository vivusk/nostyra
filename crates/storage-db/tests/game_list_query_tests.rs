//! Library list: search (Unicode), sorts, filters, summary counts.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use common::{db_with_folder, list_query as query, now, play, scanned};
use storage_db::{
    library_summary, list_games, upsert_scanned_batch, Connection, GameFilter, GameListQuery,
    GameSort, LibrarySummary,
};

/// Four games: Chrono (played long ago, 10 h), Pokémon (played now, 1 h),
/// Mother 2 Japanese title (unplayed), Zelda (unplayed, needs review).
fn seeded() -> Connection {
    let (mut conn, folder) = db_with_folder();
    let games = vec![
        scanned(folder, "chrono.sfc", "C1", "Chrono Trigger", true),
        scanned(folder, "poke.sfc", "P1", "Pokémon Stadium", true),
        scanned(folder, "mother2.sfc", "M1", "マザー2 ギーグの逆襲", true),
        scanned(folder, "zelda.smc", "Z1", "Zelda", false),
    ];
    upsert_scanned_batch(&mut conn, &games, 1).unwrap();
    play(&conn, "chrono.sfc", 1_000, 36_000);
    play(&conn, "poke.sfc", now(), 3_600);
    conn
}

fn titles(conn: &Connection, query: GameListQuery) -> Vec<String> {
    list_games(conn, &query)
        .unwrap()
        .into_iter()
        .map(|g| g.title)
        .collect()
}

#[test]
fn sorts() {
    let conn = seeded();
    assert_eq!(
        titles(&conn, query(None, GameSort::Title, GameFilter::All)),
        [
            "Chrono Trigger",
            "Pokémon Stadium",
            "Zelda",
            "マザー2 ギーグの逆襲"
        ]
    );
    assert_eq!(
        titles(
            &conn,
            query(None, GameSort::RecentlyPlayed, GameFilter::All)
        ),
        [
            "Pokémon Stadium",
            "Chrono Trigger",
            "Zelda",
            "マザー2 ギーグの逆襲"
        ]
    );
    assert_eq!(
        titles(&conn, query(None, GameSort::PlayTime, GameFilter::All))[..2],
        ["Chrono Trigger", "Pokémon Stadium"]
    );
}

#[test]
fn unicode_case_insensitive_search() {
    let conn = seeded();
    for needle in ["POKÉMON", "pokémon", "kÉm"] {
        assert_eq!(
            titles(&conn, query(Some(needle), GameSort::Title, GameFilter::All)),
            ["Pokémon Stadium"]
        );
    }
    assert_eq!(
        titles(
            &conn,
            query(Some("ギーグ"), GameSort::Title, GameFilter::All)
        ),
        ["マザー2 ギーグの逆襲"]
    );
    assert!(titles(&conn, query(Some("%"), GameSort::Title, GameFilter::All)).is_empty());
    assert_eq!(
        titles(&conn, query(Some("  "), GameSort::Title, GameFilter::All)).len(),
        4
    );
}

#[test]
fn filters() {
    let conn = seeded();
    assert_eq!(
        titles(
            &conn,
            query(None, GameSort::Title, GameFilter::RecentlyPlayed)
        ),
        ["Pokémon Stadium"]
    );
    assert_eq!(
        titles(&conn, query(None, GameSort::Title, GameFilter::Unplayed)),
        ["Zelda", "マザー2 ギーグの逆襲"]
    );
    assert_eq!(
        titles(&conn, query(None, GameSort::Title, GameFilter::NeedsReview)),
        ["Zelda"]
    );
}

#[test]
fn list_carries_play_stats_and_summary_counts() {
    let conn = seeded();
    let chrono = list_games(
        &conn,
        &query(Some("chrono"), GameSort::Title, GameFilter::All),
    )
    .unwrap();
    assert_eq!(
        (chrono[0].total_play_s, chrono[0].last_played_at),
        (36_000, Some(1_000))
    );
    assert_eq!(
        library_summary(&conn).unwrap(),
        LibrarySummary {
            total: 4,
            matched: 3,
            needs_review: 1,
            missing: 0,
        }
    );
}

#[test]
fn query_deserializes_from_ui_json() {
    let q: GameListQuery =
        serde_json::from_str(r#"{"search":"x","sort":"play-time","filter":"needs-review"}"#)
            .unwrap();
    assert_eq!(
        q,
        query(Some("x"), GameSort::PlayTime, GameFilter::NeedsReview)
    );
    let empty: GameListQuery = serde_json::from_str("{}").unwrap();
    assert_eq!(empty, GameListQuery::default());
}

#[test]
fn platform_filter_and_counts() {
    let mut conn = seeded();
    let folder = storage_db::list_library_folders(&conn).unwrap()[0].id;
    let mario = storage_db::ScannedGame {
        platform_id: "nes".into(),
        ..scanned(folder, "mario.nes", "N1", "Super Mario Bros.", true)
    };
    upsert_scanned_batch(&mut conn, &[mario], 1).unwrap();
    let nes_only = GameListQuery {
        platform: Some("nes".into()),
        ..GameListQuery::default()
    };
    assert_eq!(titles(&conn, nes_only), ["Super Mario Bros."]);
    let snes_needing_review = GameListQuery {
        platform: Some("snes".into()),
        filter: GameFilter::NeedsReview,
        ..GameListQuery::default()
    };
    assert_eq!(titles(&conn, snes_needing_review), ["Zelda"]);
    assert_eq!(
        storage_db::library_platform_counts(&conn).unwrap(),
        [("nes".to_string(), 1), ("snes".to_string(), 4)]
    );
    let from_ui: GameListQuery = serde_json::from_str(r#"{"platform":"nes"}"#).unwrap();
    assert_eq!(from_ui.platform.as_deref(), Some("nes"));
}
