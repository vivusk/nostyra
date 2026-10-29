//! Two-pass scan: a file moved between library folders relinks to its old row
//! whatever the folder order, and a mixed tree is classified per file by content.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::{fake_rom, TempLibrary};
use game_library::{game_content_sha1, scan_library, ScanSummary, SystemCatalogs};
use storage_db::{game_play_stats, list_games, GameListQuery, LibraryFolder, NewPlaySession};

/// The temp library plus a second folder `roms2/`, both registered.
fn two_folders(tag: &str) -> (TempLibrary, LibraryFolder, PathBuf) {
    let lib = TempLibrary::new(tag);
    let second = lib.root.join("roms2");
    std::fs::create_dir_all(&second).unwrap();
    let folder2 =
        storage_db::add_library_folder(&lib.db.lock().unwrap(), second.to_str().unwrap(), 1)
            .unwrap();
    (lib, folder2, second)
}

fn scan(lib: &TempLibrary, folders: &[LibraryFolder]) -> ScanSummary {
    scan_library(
        &lib.db,
        folders,
        &SystemCatalogs::new(None),
        10,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap()
}

/// 32 KiB GB/GBC image with the logo bytes mGBA checks (or without, `logo` false).
fn gb_image(seed: u32, logo: bool) -> Vec<u8> {
    let mut rom = fake_rom(32 * 1024, seed);
    rom[0x104..0x108].copy_from_slice(if logo {
        &[0xCE, 0xED, 0x66, 0x66]
    } else {
        &[0; 4]
    });
    rom
}

/// 64 KiB GBA image whose byte 3 is the ARM branch 0xEA (or not).
fn gba_image(seed: u32, branch: bool) -> Vec<u8> {
    let mut rom = fake_rom(64 * 1024, seed);
    rom[3] = if branch { 0xEA } else { 0 };
    rom
}

fn ines() -> Vec<u8> {
    let mut rom = vec![b'N', b'E', b'S', 0x1A, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    rom.extend((0..24_576u32).map(|i| (i * 7 % 251) as u8));
    rom
}

/// Moves `game.sfc` into the folder scanned first (or last); history must follow.
fn move_keeps_history(tag: &str, move_into_first_scanned: bool) {
    let (lib, folder2, second) = two_folders(tag);
    let folders = [lib.folder.clone(), folder2];
    let rom = fake_rom(64 * 1024, 7);
    let (old_dir, new_dir) = if move_into_first_scanned {
        (second.clone(), lib.roms())
    } else {
        (lib.roms(), second.clone())
    };
    std::fs::write(old_dir.join("game.sfc"), &rom).unwrap();
    assert_eq!(scan(&lib, &folders).added, 1);
    let id = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap()[0].id;
    let session = NewPlaySession {
        game_id: id,
        started_at: 5,
        duration_s: 90,
        exit_code: Some(0),
        source_key: None,
    };
    storage_db::record_play_session(&lib.db.lock().unwrap(), &session).unwrap();
    storage_db::set_favorite(&lib.db.lock().unwrap(), id, true).unwrap();

    std::fs::rename(old_dir.join("game.sfc"), new_dir.join("game.sfc")).unwrap();
    let summary = scan(&lib, &folders);
    assert_eq!((summary.added, summary.relinked), (0, 1), "{tag}");
    assert_eq!(summary.relinked_games[0].game_id, id);
    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    assert_eq!(games.len(), 1, "no duplicate row");
    assert!(games[0].favorite && !games[0].missing);
    assert!(games[0].rom_path.starts_with(new_dir.to_str().unwrap()));
    let stats = game_play_stats(&lib.db.lock().unwrap(), id).unwrap();
    assert_eq!(stats.total_play_s, 90);
}

#[test]
fn move_into_the_folder_scanned_first_relinks() {
    move_keeps_history("move-first", true);
}

#[test]
fn move_into_the_folder_scanned_last_relinks() {
    move_keeps_history("move-last", false);
}

#[test]
fn mixed_tree_scans_each_file_to_its_system() {
    let lib = TempLibrary::new("mixed");
    lib.write("SNES/a.sfc", &fake_rom(64 * 1024, 1));
    lib.write("NES/b.nes", &ines());
    lib.write("NES/c.sfc", &fake_rom(64 * 1024, 2));
    lib.write("Favorites/d.nes", &{
        let mut rom = ines();
        rom[200] ^= 1;
        rom
    });
    lib.write("NES/readme.txt", b"junk");
    lib.write("NES/bad.nes", &ines()[16..]);
    // M7 handhelds: one folder of mixed carts, a renamed .gbc, and two bad files.
    lib.write("Handheld/e.gb", &gb_image(3, true));
    lib.write("Handheld/f.gbc", &gb_image(4, true));
    lib.write("Handheld/g.GBA", &gba_image(5, true));
    lib.write("SNES/h.gb", &gb_image(6, true));
    lib.write("Handheld/no-logo.gb", &gb_image(7, false));
    lib.write("Handheld/no-branch.gba", &gba_image(8, false));
    common::ps1_test_tree::write_ps1_tree(&lib);
    let summary = scan(&lib, std::slice::from_ref(&lib.folder));
    assert_eq!(summary.added, 8 + 4, "8 carts + 4 PS1 games");
    let mut failed: Vec<(String, &str)> = summary
        .failures
        .iter()
        .map(|f| {
            let name = f.path.file_name().unwrap().to_string_lossy().into_owned();
            (name, f.reason.as_str())
        })
        .collect();
    failed.sort();
    let expected = [
        ("Brig_TGGR.cue", common::ps1_test_tree::BROKEN_REASON),
        ("audio.bin", "not a PlayStation disc"),
        ("bad.nes", "missing iNES header"),
        ("no-branch.gba", "missing GBA header"),
        ("no-logo.gb", "missing Game Boy header"),
        ("ps2.bin", "not a PlayStation disc"),
    ];
    assert_eq!(
        failed,
        expected.map(|(name, reason)| (name.to_string(), reason))
    );
    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    let count = |id: &str| games.iter().filter(|g| g.platform_id == id).count();
    assert_eq!((count("snes"), count("nes")), (2, 2));
    assert_eq!((count("gb"), count("gbc"), count("gba")), (2, 1, 1));
    let mut psx: Vec<&str> = games
        .iter()
        .filter(|g| g.platform_id == "psx")
        .map(|g| g.rom_path.rsplit(['/', '\\']).next().unwrap())
        .collect();
    psx.sort_unstable();
    assert_eq!(psx, ["Bare.bin", "Game.cue", "Multi.m3u", "Plain.iso"]);
    let c = games
        .iter()
        .find(|g| g.rom_path.ends_with("c.sfc"))
        .unwrap();
    assert_eq!(c.platform_id, "snes", "an .sfc in NES/ is SNES");
    // The runner's save-folder key (no launcher) is exactly the stored sha1.
    for game in &games {
        let system = system_catalog::system(&game.platform_id).unwrap();
        let path = std::path::Path::new(&game.rom_path);
        assert_eq!(
            game_content_sha1(path, system).unwrap(),
            game.sha1,
            "{}",
            game.rom_path
        );
    }
}
