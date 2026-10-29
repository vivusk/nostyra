//! `fetch_game_artwork` with an offline client: every outcome must come back
//! immediately, without a socket, files or panics.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use artwork_cache::{fetch_game_artwork, ArtworkKind, GameArtRequest, KindOutcome, ThumbnailIndex};
use http_download_util::build_http_client;
use support::{file_names, fresh_dir, SAMPLE_TREE_JSON};

fn request(name: &str, kinds: Vec<ArtworkKind>) -> GameArtRequest {
    GameArtRequest {
        game_id: 7,
        system: &system_catalog::SNES,
        no_intro_name: name.into(),
        kinds,
    }
}

#[test]
fn offline_without_index_is_transient_for_every_kind() {
    let temp = fresh_dir("offline-no-index");
    let dir = temp.path();
    let http = build_http_client("Nostyra-test", true);
    let kinds = vec![ArtworkKind::Cover, ArtworkKind::Hero, ArtworkKind::Cover];
    let started = Instant::now();
    let outcome = fetch_game_artwork(
        &http,
        None,
        &request("Chrono Trigger (USA)", kinds),
        dir,
        &AtomicBool::new(false),
    );
    assert!(started.elapsed() < Duration::from_millis(200));
    let offline = KindOutcome::Transient("offline".into());
    assert_eq!(
        outcome.results,
        vec![
            (ArtworkKind::Cover, offline.clone()),
            (ArtworkKind::Hero, offline)
        ]
    );
    assert!(outcome.is_transient());
    assert!(!outcome.network);
    assert!(file_names(dir).is_empty());
}

#[test]
fn index_miss_is_not_found_without_network() {
    let temp = fresh_dir("offline-index-miss");
    let dir = temp.path();
    let http = build_http_client("Nostyra-test", true);
    let index = ThumbnailIndex::parse_tree_json(SAMPLE_TREE_JSON).unwrap();
    let outcome = fetch_game_artwork(
        &http,
        Some(&index),
        &request(
            "Unknown Game (USA)",
            vec![ArtworkKind::Cover, ArtworkKind::Hero],
        ),
        dir,
        &AtomicBool::new(false),
    );
    assert_eq!(
        outcome.get(ArtworkKind::Cover),
        Some(&KindOutcome::NotFound)
    );
    assert_eq!(outcome.get(ArtworkKind::Hero), Some(&KindOutcome::NotFound));
    assert!(!outcome.is_transient());
    assert!(
        !outcome.network,
        "index-derived misses never touch the network"
    );
}

#[test]
fn index_hit_while_offline_is_transient() {
    let temp = fresh_dir("offline-index-hit");
    let dir = temp.path();
    let http = build_http_client("Nostyra-test", true);
    let index = ThumbnailIndex::parse_tree_json(SAMPLE_TREE_JSON).unwrap();
    // Hero exists only as a title screen: the snap miss must fall through to it.
    let outcome = fetch_game_artwork(
        &http,
        Some(&index),
        &request("Super Mario World (USA) (Rev 1)", vec![ArtworkKind::Hero]),
        dir,
        &AtomicBool::new(false),
    );
    assert_eq!(
        outcome.get(ArtworkKind::Hero),
        Some(&KindOutcome::Transient("offline".into()))
    );
    assert!(outcome.is_transient());
}

#[test]
fn cancel_flag_short_circuits() {
    let temp = fresh_dir("offline-cancel");
    let dir = temp.path();
    let http = build_http_client("Nostyra-test", true);
    let outcome = fetch_game_artwork(
        &http,
        None,
        &request(
            "Chrono Trigger (USA)",
            vec![ArtworkKind::Cover, ArtworkKind::Hero],
        ),
        dir,
        &AtomicBool::new(true),
    );
    assert_eq!(outcome.results.len(), 2);
    assert!(outcome
        .results
        .iter()
        .all(|(_, result)| *result == KindOutcome::Cancelled));
    assert!(!outcome.is_transient());
}

#[test]
fn kind_names_and_file_names_are_stable() {
    assert_eq!(ArtworkKind::Cover.as_str(), "cover");
    assert_eq!(ArtworkKind::Hero.as_str(), "hero");
    assert_eq!(ArtworkKind::Cover.file_name(42), "42-cover.jpg");
    assert_eq!(ArtworkKind::Hero.file_name(42), "42-hero.png");
}
