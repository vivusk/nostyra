//! Cache maintenance: orphan tmp sweep, size, clear (index survives).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use artwork_cache::{artwork_cache_size, clear_artwork_cache, sweep_orphan_tmp, ArtworkCacheSize};
use support::{file_names, fresh_dir};

fn seed(test: &str) -> tempfile::TempDir {
    let temp = fresh_dir(test);
    let dir = temp.path();
    std::fs::create_dir_all(dir.join("index")).unwrap();
    std::fs::write(dir.join("1-cover.jpg"), [0u8; 100]).unwrap();
    std::fs::write(dir.join("1-hero.png"), [0u8; 20]).unwrap();
    std::fs::write(dir.join("2-cover.jpg.tmp"), [0u8; 7]).unwrap();
    std::fs::write(dir.join("index/snes-tree.json"), b"{}").unwrap();
    std::fs::write(dir.join("index/snes-tree.json.tmp"), b"{").unwrap();
    temp
}

#[test]
fn sweep_removes_only_tmp_files() {
    let temp = seed("sweep");
    let dir = temp.path();
    assert_eq!(sweep_orphan_tmp(dir), 2);
    assert_eq!(file_names(dir), vec!["1-cover.jpg", "1-hero.png", "index"]);
    assert_eq!(file_names(&dir.join("index")), vec!["snes-tree.json"]);
}

#[test]
fn size_counts_art_files_only() {
    let temp = seed("size");
    let dir = temp.path();
    assert_eq!(
        artwork_cache_size(dir),
        ArtworkCacheSize {
            files: 2,
            bytes: 120
        }
    );
}

#[test]
fn clear_deletes_art_and_keeps_the_index() {
    let temp = seed("clear");
    let dir = temp.path();
    assert_eq!(clear_artwork_cache(dir).unwrap(), 3);
    assert_eq!(file_names(dir), vec!["index"]);
    assert!(dir.join("index/snes-tree.json").is_file());
    assert_eq!(artwork_cache_size(dir), ArtworkCacheSize::default());
}

#[test]
fn missing_directory_is_an_empty_cache() {
    let temp = fresh_dir("missing");
    let dir = temp.path().join("never-created");
    assert_eq!(artwork_cache_size(&dir), ArtworkCacheSize::default());
    assert_eq!(clear_artwork_cache(&dir).unwrap(), 0);
    assert_eq!(sweep_orphan_tmp(&dir), 0);
}
