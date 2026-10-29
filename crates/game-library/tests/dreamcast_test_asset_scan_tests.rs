//! Scan of the committed Dreamcast test discs (`test-assets/roms/nostyra-test-dc*`).
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use game_library::{hash_file_streaming, scan_library, SystemCatalogs};
use storage_db::{list_games, Game, GameListQuery};

fn scan(lib: &TempLibrary) -> Vec<Game> {
    let catalogs = SystemCatalogs::new(None);
    let summary = scan_library(
        &lib.db,
        std::slice::from_ref(&lib.folder),
        &catalogs,
        10,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap()
}

/// The committed test discs (`build-nostyra-test-dreamcast.py`): the Redump-style
/// `.cue` and the two-disc `.m3u` are one row each (both `.gdi` files are members of
/// the playlist, every track hidden), identified by Track 3 and IP.BIN.
#[test]
fn committed_test_discs_scan_as_cue_and_playlist() {
    let lib = TempLibrary::new("dc-assets");
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms");
    for entry in std::fs::read_dir(&assets).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        if name.starts_with("nostyra-test-dc") {
            std::fs::copy(&path, lib.roms().join(&name)).unwrap();
        }
    }
    let rows = scan(&lib);
    let file_name = |g: &Game| {
        let path = std::path::Path::new(&g.rom_path);
        path.file_name().unwrap().to_str().unwrap().to_owned()
    };
    let mut names: Vec<String> = rows.iter().map(file_name).collect();
    names.sort_unstable();
    assert_eq!(names, ["nostyra-test-dc.cue", "nostyra-test-dc.m3u"]);
    let track3 = hash_file_streaming(&lib.roms().join("nostyra-test-dc (Track 3).bin")).unwrap();
    for game in &rows {
        assert_eq!(game.platform_id, "dreamcast");
        assert_eq!(
            game.serial.as_deref(),
            Some("NSTY-00001"),
            "{}",
            game.rom_path
        );
        assert_eq!(game.sha1, track3.sha1, "{}", game.rom_path);
    }
}
