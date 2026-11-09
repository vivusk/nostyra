//! DAT-gated loose `.bin` (M17): a `.bin` neither Mega Drive nor PlayStation / PS2 takes is
//! listed as Atari 2600 / 7800 only when the system's DAT knows it; a miss keeps the
//! old reason plus a hint. Disc tracks never become pending.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use game_library::{
    dat_pending_hint, discover_rom_files, hash_rom_bytes, scan_library, ScanSummary, SystemCatalogs,
};
use storage_db::{list_games, GameListQuery};
use system_catalog::{SystemSpec, ALL_SYSTEMS, ATARI2600, ATARI7800, GENESIS};

fn image(len: u32, seed: u32) -> Vec<u8> {
    (0..len)
        .map(|i| (i.wrapping_mul(seed) % 251) as u8)
        .collect()
}

fn write_dat(dir: &Path, spec: &SystemSpec, game: &str, rom: &[u8]) {
    let hash = hash_rom_bytes(rom, spec);
    let dat = format!(
        "game (\n\tname \"{game}\"\n\trom ( name \"{game}.bin\" size {} crc {} sha1 {} )\n)\n",
        hash.size, hash.crc32, hash.sha1
    );
    std::fs::write(dir.join(spec.dat_file), dat).unwrap();
}

fn scan(lib: &TempLibrary, catalogs: &SystemCatalogs) -> ScanSummary {
    scan_library(
        &lib.db,
        std::slice::from_ref(&lib.folder),
        catalogs,
        10,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap()
}

#[test]
fn dat_listed_bins_become_atari_games_and_misses_get_the_hint() {
    let db = tempfile::Builder::new()
        .prefix("nostyra-gl-atari-bin-")
        .tempdir()
        .unwrap();
    let (vcs, prosystem) = (image(4096, 7), image(16384, 11));
    write_dat(db.path(), &ATARI2600, "Fake Pitfall (USA)", &vcs);
    write_dat(db.path(), &ATARI7800, "Fake Asteroids (USA)", &prosystem);
    let mut md = image(0x20000, 3);
    md[0x100..0x104].copy_from_slice(b"SEGA");

    let lib = TempLibrary::new("atari-bin");
    lib.write("Mixed/pitfall.bin", &vcs);
    lib.write("Mixed/asteroids.bin", &prosystem);
    lib.write("Mixed/homebrew.bin", &image(4096, 13));
    lib.write("Mixed/sonic.bin", &md);
    let catalogs = SystemCatalogs::new(Some(db.path().to_path_buf()));
    let summary = scan(&lib, &catalogs);
    assert_eq!(summary.added, 3);
    let reasons: Vec<(String, &str)> = summary
        .failures
        .iter()
        .map(|f| {
            (
                f.path.file_name().unwrap().to_string_lossy().into_owned(),
                f.reason.as_str(),
            )
        })
        .collect();
    let hint = dat_pending_hint(
        "not a PlayStation or PlayStation 2 disc",
        &[&ATARI2600, &ATARI7800],
    );
    assert_eq!(reasons, [("homebrew.bin".to_string(), hint.as_str())]);
    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    for (file, system, name) in [
        ("pitfall.bin", "atari2600", Some("Fake Pitfall (USA)")),
        ("asteroids.bin", "atari7800", Some("Fake Asteroids (USA)")),
        ("sonic.bin", "genesis", None),
    ] {
        let game = games.iter().find(|g| g.rom_path.ends_with(file)).unwrap();
        assert_eq!(
            (game.platform_id.as_str(), game.dat_name.as_deref()),
            (system, name)
        );
    }

    // Rescan: the hits take the unchanged path, nothing goes missing.
    let again = scan(&lib, &catalogs);
    assert_eq!((again.added, again.unchanged, again.missing), (0, 3, 0));
    // A renamed hit relinks (same content id, same row).
    std::fs::rename(
        lib.roms().join("Mixed/pitfall.bin"),
        lib.roms().join("Mixed/pit.bin"),
    )
    .unwrap();
    let renamed = scan(&lib, &catalogs);
    assert_eq!((renamed.added, renamed.relinked), (0, 1));
    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    let pit = games
        .iter()
        .find(|g| g.rom_path.ends_with("pit.bin"))
        .unwrap();
    assert_eq!(pit.platform_id, "atari2600");
}

#[test]
fn cartridge_only_scans_still_split_pending_bins() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-gl-atari-cart-")
        .tempdir()
        .unwrap();
    std::fs::write(temp.path().join("x.bin"), image(4096, 5)).unwrap();
    let found = discover_rom_files(temp.path(), &[&GENESIS, &ATARI2600, &ATARI7800]);
    assert!(found.failures.is_empty(), "{:?}", found.failures);
    assert_eq!(found.dat_pending.len(), 1);
    assert_eq!(found.dat_pending[0].reason, "missing Mega Drive header");
    let ids: Vec<&str> = found.dat_pending[0]
        .candidates
        .iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(ids, ["atari2600", "atari7800"]);
}

#[test]
fn test_asset_disc_tracks_are_never_pending() {
    let roms = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms");
    let found = discover_rom_files(&roms, &ALL_SYSTEMS);
    // Only the headerless 7800 test ROM waits for a DAT; no DC / PSX track does.
    let pending: Vec<&str> = found.dat_pending.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(pending.len(), 1, "{pending:?}");
    assert!(pending[0].ends_with("nostyra-test-7800.bin"), "{pending:?}");
    let systems: Vec<&str> = found.files.iter().map(|f| f.system.id).collect();
    for disc in ["psx", "dreamcast"] {
        assert!(systems.contains(&disc), "{disc} still listed");
    }
}
