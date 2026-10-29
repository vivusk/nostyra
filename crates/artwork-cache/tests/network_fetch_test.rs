//! Real download from libretro-thumbnails. Ignored by default (needs network,
//! spends one GitHub API request): `cargo test -p artwork-cache -- --ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::sync::atomic::AtomicBool;
use std::time::{Instant, SystemTime};

use artwork_cache::{
    fetch_game_artwork, ArtworkKind, GameArtRequest, KindOutcome, ThumbnailFolder, ThumbnailIndex,
};
use http_download_util::build_http_client;
use image::{GenericImageView, ImageFormat};
use support::fresh_dir;

const USER_AGENT: &str = "Nostyra/0.1 (artwork-cache network test)";

fn chrono_trigger() -> GameArtRequest {
    GameArtRequest {
        game_id: 1,
        system: &system_catalog::SNES,
        no_intro_name: "Chrono Trigger (USA)".into(),
        kinds: vec![ArtworkKind::Cover, ArtworkKind::Hero],
    }
}

/// Asserts both kinds were cached and checks the files' formats and sizes.
fn assert_cached(dir: &std::path::Path, outcome: &artwork_cache::ArtworkOutcome) {
    assert!(outcome.network);
    for (kind, result) in &outcome.results {
        assert!(
            matches!(result, KindOutcome::Cached { .. }),
            "{kind:?}: {result:?}"
        );
    }
    let cover = std::fs::read(dir.join("1-cover.jpg")).unwrap();
    assert_eq!(image::guess_format(&cover).unwrap(), ImageFormat::Jpeg);
    let dims = image::load_from_memory(&cover).unwrap().dimensions();
    println!("cover {} bytes, {dims:?}", cover.len());
    assert!(dims.0 <= 600 && dims.1 <= 600);
    let hero = std::fs::read(dir.join("1-hero.png")).unwrap();
    assert_eq!(image::guess_format(&hero).unwrap(), ImageFormat::Png);
    println!("hero {} bytes", hero.len());
}

#[test]
#[ignore = "network: downloads from GitHub and thumbnails.libretro.com"]
fn fetches_chrono_trigger_cover_and_hero() {
    let http = build_http_client(USER_AGENT, false);
    let cancel = AtomicBool::new(false);

    // Index path (one GitHub API call, then cached under index/).
    let temp = fresh_dir("network-index");
    let dir = temp.path();
    let started = Instant::now();
    let index = ThumbnailIndex::load_or_fetch(&http, dir, &system_catalog::SNES, SystemTime::now())
        .expect("GitHub tree index (rate limit is 60/h per IP)");
    println!(
        "index: {} boxarts in {:?}",
        index.len(ThumbnailFolder::Boxarts),
        started.elapsed()
    );
    assert!(dir.join("index/snes-tree.json").is_file());
    let started = Instant::now();
    let outcome = fetch_game_artwork(&http, Some(&index), &chrono_trigger(), dir, &cancel);
    println!("indexed fetch: {outcome:?} in {:?}", started.elapsed());
    assert_cached(dir, &outcome);

    // Direct-GET path (no index): same result.
    let temp = fresh_dir("network-direct");
    let dir = temp.path();
    let started = Instant::now();
    let outcome = fetch_game_artwork(&http, None, &chrono_trigger(), dir, &cancel);
    println!("direct fetch: {outcome:?} in {:?}", started.elapsed());
    assert_cached(dir, &outcome);
}
