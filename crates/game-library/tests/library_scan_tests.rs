//! End-to-end folder scans against a real (temp file) storage-db.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use common::{fake_rom, test_rom_bytes, TempLibrary};
use game_library::{
    hash_rom_bytes, scan_library, DatEntry, DatIndex, ScanProgress, SystemCatalogs,
};
use storage_db::{list_games, GameListQuery, GameSort};

fn listed(lib: &TempLibrary) -> Vec<storage_db::Game> {
    let query = GameListQuery {
        sort: GameSort::Title,
        ..GameListQuery::default()
    };
    list_games(&lib.db.lock().unwrap(), &query).unwrap()
}

#[test]
fn identifies_known_dumps_and_lists_unknown_as_needs_review() {
    let lib = TempLibrary::new("identify");
    let known = fake_rom(64 * 1024, 1);
    let hashes = hash_rom_bytes(&known, &system_catalog::SNES);
    let dat = DatIndex::from_entries(vec![DatEntry {
        name: "Fake Quest (USA) (Rev 1)".into(),
        region: None,
        size: hashes.size,
        crc32: hashes.crc32.clone(),
        metadata_crc32: hashes.crc32.clone(),
        serial: None,
        sha1: Some(hashes.sha1.clone()),
    }]);
    // Same dump with a 512-byte copier header as .smc → still identified.
    let mut headered = vec![0u8; 512];
    headered.extend_from_slice(&known);
    lib.write("Fake Quest (U).smc", &headered);
    lib.write("homebrew_demo_v2.sfc", &test_rom_bytes());
    lib.write("tiny.sfc", &[0u8; 1024]); // below 32 KiB → skipped
    lib.write("readme.txt", b"not a rom");

    let summary = lib.scan(&dat);
    assert_eq!((summary.added, summary.skipped), (2, 1));
    assert_eq!((summary.matched, summary.unmatched), (1, 1));
    assert!(summary.failures.is_empty());

    let games = listed(&lib);
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].title, "Fake Quest");
    assert_eq!(games[0].region.as_deref(), Some("USA"));
    assert_eq!(
        games[0].dat_name.as_deref(),
        Some("Fake Quest (USA) (Rev 1)")
    );
    assert_eq!(games[0].sha1, hashes.sha1);
    assert_eq!(games[1].title, "homebrew_demo_v2");
    assert!(games[1].needs_review());
}

#[test]
fn rescan_skips_unchanged_marks_missing_and_relinks_moves() {
    let lib = TempLibrary::new("rescan");
    let dat = DatIndex::empty();
    lib.write("a.sfc", &fake_rom(40_000, 1));
    lib.write("b.sfc", &fake_rom(40_000, 2));
    lib.write("c.sfc", &fake_rom(40_000, 3));
    assert_eq!(lib.scan(&dat).added, 3);

    let again = lib.scan(&dat);
    assert_eq!((again.unchanged, again.added, again.updated), (3, 0, 0));

    std::fs::remove_file(lib.roms().join("b.sfc")).unwrap();
    std::fs::create_dir_all(lib.roms().join("moved")).unwrap();
    std::fs::rename(
        lib.roms().join("c.sfc"),
        lib.roms().join("moved/c renamed.sfc"),
    )
    .unwrap();
    let after = lib.scan(&dat);
    assert_eq!((after.missing, after.relinked, after.added), (2, 1, 0));

    let games = listed(&lib);
    let b = games
        .iter()
        .find(|g| g.rom_path.ends_with("b.sfc"))
        .unwrap();
    assert!(b.missing);
    assert!(games
        .iter()
        .any(|g| g.rom_path.ends_with("c renamed.sfc") && !g.missing));
    assert_eq!(games.len(), 3);
}

#[test]
fn rescan_identifies_unchanged_games_once_the_dat_knows_them() {
    let lib = TempLibrary::new("rematch");
    let rom = fake_rom(64 * 1024, 7);
    lib.write("mystery.sfc", &rom);
    assert_eq!(lib.scan(&DatIndex::empty()).unmatched, 1);
    assert!(listed(&lib)[0].needs_review());

    let hashes = hash_rom_bytes(&rom, &system_catalog::SNES);
    let dat = DatIndex::from_entries(vec![DatEntry {
        name: "Mystery Quest (Japan)".into(),
        region: None,
        size: hashes.size,
        crc32: hashes.crc32.clone(),
        metadata_crc32: hashes.crc32,
        serial: None,
        sha1: Some(hashes.sha1),
    }]);
    let summary = lib.scan(&dat);
    assert_eq!((summary.unchanged, summary.rematched), (1, 1));
    let games = listed(&lib);
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].title, "Mystery Quest");
    assert_eq!(games[0].dat_name.as_deref(), Some("Mystery Quest (Japan)"));
    assert_eq!(lib.scan(&dat).rematched, 0);
}

#[test]
fn reports_progress_and_honours_cancel() {
    let lib = TempLibrary::new("progress");
    for i in 0..120 {
        lib.write(&format!("g{i}.sfc"), &fake_rom(32 * 1024, i));
    }
    let seen = Mutex::new(Vec::<ScanProgress>::new());
    let summary = scan_library(
        &lib.db,
        std::slice::from_ref(&lib.folder),
        &SystemCatalogs::new(None),
        10,
        &|p| seen.lock().unwrap().push(p.clone()),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(summary.added, 120);
    let seen = seen.into_inner().unwrap();
    assert_eq!(seen.first().unwrap().files_done, 0);
    let last = seen.last().unwrap();
    assert_eq!((last.files_total, last.files_done), (120, 120));

    let cancelled = TempLibrary::new("cancel");
    cancelled.write("x.sfc", &fake_rom(32 * 1024, 9));
    let summary = scan_library(
        &cancelled.db,
        std::slice::from_ref(&cancelled.folder),
        &SystemCatalogs::new(None),
        10,
        &|_| {},
        &AtomicBool::new(true),
    )
    .unwrap();
    assert!(summary.cancelled);
    assert_eq!(summary.added, 0);
}

#[test]
fn missing_folder_root_is_reported_and_other_folders_still_scan() {
    let lib = TempLibrary::new("nofolder");
    lib.write("a.sfc", &fake_rom(32 * 1024, 1));
    let second = lib.root.join("roms2");
    std::fs::create_dir_all(&second).unwrap();
    std::fs::write(second.join("b.sfc"), fake_rom(32 * 1024, 2)).unwrap();
    let folder2 =
        storage_db::add_library_folder(&lib.db.lock().unwrap(), second.to_str().unwrap(), 1)
            .unwrap();
    let folders = [lib.folder.clone(), folder2.clone()];
    let scan = |seen: &Mutex<Vec<ScanProgress>>| {
        scan_library(
            &lib.db,
            &folders,
            &SystemCatalogs::new(None),
            10,
            &|p| seen.lock().unwrap().push(p.clone()),
            &AtomicBool::new(false),
        )
        .unwrap()
    };
    assert_eq!(scan(&Mutex::new(Vec::new())).added, 2);

    std::fs::remove_dir_all(lib.roms()).unwrap();
    std::fs::write(second.join("c.sfc"), fake_rom(32 * 1024, 3)).unwrap();
    let seen = Mutex::new(Vec::new());
    let summary = scan(&seen);
    assert_eq!(summary.added, 1, "the present folder is scanned");
    assert_eq!(
        summary.missing, 0,
        "rows of the missing folder are left as they were"
    );
    assert_eq!(summary.folder_errors.len(), 1);
    assert_eq!(summary.folder_errors[0].0, lib.folder.path);
    assert!(listed(&lib).iter().all(|game| !game.missing));
    let seen = seen.into_inner().unwrap();
    assert!(seen.iter().any(|p| p.folder_path == lib.folder.path));
    assert!(seen
        .iter()
        .any(|p| p.folder_path == folder2.path && p.files_total == 2));
}
