//! Tree JSON parsing and the cache refresh rule, driven by a fake clock and an
//! offline client (no test here may touch the network).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::time::{Duration, SystemTime};

use artwork_cache::{index_cache_is_fresh, ThumbnailFolder, ThumbnailIndex, INDEX_MAX_AGE};
use http_download_util::build_http_client;
use support::{fresh_dir, SAMPLE_TREE_JSON};

#[test]
fn parses_the_three_folders_and_ignores_the_rest() {
    let index = ThumbnailIndex::parse_tree_json(SAMPLE_TREE_JSON).unwrap();
    assert_eq!(index.len(ThumbnailFolder::Boxarts), 3);
    assert_eq!(index.len(ThumbnailFolder::Snaps), 1);
    assert_eq!(index.len(ThumbnailFolder::Titles), 2);
    assert!(index.contains(ThumbnailFolder::Boxarts, "Q_bert's Qubes (USA)"));
    // Nested folder copies and non-PNG files are not servable thumbnails.
    assert!(!index.contains(ThumbnailFolder::Boxarts, "Clock Tower (USA)"));
    assert!(!index.contains(ThumbnailFolder::Boxarts, "Notes"));
    assert!(!index.contains(ThumbnailFolder::Snaps, "Super Mario World (USA)"));
}

#[test]
fn find_name_returns_the_best_listed_candidate() {
    let index = ThumbnailIndex::parse_tree_json(SAMPLE_TREE_JSON).unwrap();
    let candidates = vec![
        "Chrono Trigger (USA) (Rev 1)".to_owned(),
        "Chrono Trigger (USA)".to_owned(),
    ];
    assert_eq!(
        index.find_name(ThumbnailFolder::Boxarts, &candidates),
        Some("Chrono Trigger (USA)")
    );
    let missing = vec!["Nope (USA)".to_owned()];
    assert_eq!(index.find_name(ThumbnailFolder::Boxarts, &missing), None);
}

#[test]
fn rejects_garbage_and_boxart_free_trees() {
    assert!(ThumbnailIndex::parse_tree_json(b"not json").is_err());
    let empty = br#"{"tree": [], "truncated": false}"#;
    assert!(ThumbnailIndex::parse_tree_json(empty).is_err());
}

#[test]
fn freshness_rule_is_24_hours() {
    let written = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    assert!(index_cache_is_fresh(
        written,
        written + Duration::from_secs(3600)
    ));
    assert!(!index_cache_is_fresh(written, written + INDEX_MAX_AGE));
    assert!(!index_cache_is_fresh(
        written,
        written + Duration::from_secs(48 * 3600)
    ));
    // Clock moved backwards: keep using the cache rather than refetching.
    assert!(index_cache_is_fresh(
        written,
        written - Duration::from_secs(60)
    ));
}

/// Writes the sample tree as the cached index with mtime `written`.
fn seed_cache(test: &str, written: SystemTime) -> tempfile::TempDir {
    let temp = fresh_dir(test);
    let index_dir = temp.path().join("index");
    std::fs::create_dir_all(&index_dir).unwrap();
    let path = index_dir.join("snes-tree.json");
    std::fs::write(&path, SAMPLE_TREE_JSON).unwrap();
    let file = std::fs::File::options().write(true).open(&path).unwrap();
    file.set_modified(written).unwrap();
    temp
}

#[test]
fn fresh_cache_loads_without_network() {
    let written = SystemTime::now() - Duration::from_secs(3600);
    let temp = seed_cache("index-fresh", written);
    let dir = temp.path();
    let offline = build_http_client("Nostyra-test", true);
    let index =
        ThumbnailIndex::load_or_fetch(&offline, dir, &system_catalog::SNES, SystemTime::now())
            .unwrap();
    assert!(index.contains(ThumbnailFolder::Boxarts, "Chrono Trigger (USA)"));
}

#[test]
fn stale_cache_is_still_used_when_refresh_fails() {
    let written = SystemTime::now() - Duration::from_secs(48 * 3600);
    let temp = seed_cache("index-stale", written);
    let dir = temp.path();
    let offline = build_http_client("Nostyra-test", true);
    let index =
        ThumbnailIndex::load_or_fetch(&offline, dir, &system_catalog::SNES, SystemTime::now());
    assert!(index.is_some());
    // A failed refresh must not restart the cache's 24 h window.
    let mtime = std::fs::metadata(dir.join("index/snes-tree.json"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(mtime, written);
}

#[test]
fn no_cache_and_offline_means_no_index() {
    let temp = fresh_dir("index-none");
    let dir = temp.path();
    let offline = build_http_client("Nostyra-test", true);
    assert!(
        ThumbnailIndex::load_or_fetch(&offline, dir, &system_catalog::SNES, SystemTime::now())
            .is_none()
    );
}

#[test]
fn each_system_has_its_own_index_cache() {
    // An SNES cache must not answer NES lookups: NES reads `index/nes-tree.json`.
    let temp = seed_cache("index-per-system", SystemTime::now());
    let dir = temp.path();
    let offline = build_http_client("Nostyra-test", true);
    assert!(
        ThumbnailIndex::load_or_fetch(&offline, dir, &system_catalog::NES, SystemTime::now())
            .is_none()
    );
    std::fs::copy(
        dir.join("index/snes-tree.json"),
        dir.join("index/nes-tree.json"),
    )
    .unwrap();
    assert!(
        ThumbnailIndex::load_or_fetch(&offline, dir, &system_catalog::NES, SystemTime::now())
            .is_some()
    );
}
