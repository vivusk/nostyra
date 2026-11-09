//! Single-ROM ZIPs of the Atari systems (M17): named-extension members classify,
//! hash and read like loose files; a zipped `.bin` is never DAT-gated.
#![allow(clippy::unwrap_used)]

mod common;

use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use game_library::{
    hash_rom_bytes, read_rom_archive, scan_library_with_options, ScanOptions, ScanSummary,
    SystemCatalogs,
};
use storage_db::{list_games, GameListQuery};
use system_catalog::ATARI2600;
use zip::{write::SimpleFileOptions, ZipWriter};

fn archive(path: &Path, name: &str, bytes: &[u8]) {
    let mut writer = ZipWriter::new(std::fs::File::create(path).unwrap());
    writer
        .start_file(name, SimpleFileOptions::default())
        .unwrap();
    writer.write_all(bytes).unwrap();
    writer.finish().unwrap();
}

fn scan(lib: &TempLibrary, catalogs: &SystemCatalogs) -> ScanSummary {
    scan_library_with_options(
        &lib.db,
        std::slice::from_ref(&lib.folder),
        catalogs,
        10,
        &|_| {},
        &AtomicBool::new(false),
        ScanOptions {
            scan_zip_archives: true,
        },
    )
    .unwrap()
}

fn headered(magic_at: usize, magic: &[u8], header: usize, data_len: usize) -> Vec<u8> {
    let mut file = vec![0x5Au8; header + data_len];
    file[..header].fill(0);
    file[magic_at..magic_at + magic.len()].copy_from_slice(magic);
    file
}

#[test]
fn zipped_atari_members_list_hash_and_read() {
    let lib = TempLibrary::new("zip-atari");
    let vcs: Vec<u8> = (0..4096u32).map(|i| (i % 253) as u8).collect();
    archive(&lib.roms().join("pitfall.zip"), "pitfall.a26", &vcs);
    archive(
        &lib.roms().join("asteroids.zip"),
        "asteroids.a78",
        &headered(1, b"ATARI7800", 128, 16384),
    );
    archive(
        &lib.roms().join("games.zip"),
        "games.lnx",
        &headered(0, b"LYNX\0", 64, 131_072),
    );
    archive(&lib.roms().join("odd.zip"), "odd.bin", &vec![0x11u8; 4096]);
    let summary = scan(&lib, &SystemCatalogs::new(None));
    assert_eq!(summary.added, 3);
    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    for (file, system) in [
        ("pitfall.zip", "atari2600"),
        ("asteroids.zip", "atari7800"),
        ("games.zip", "lynx"),
    ] {
        let game = games.iter().find(|g| g.rom_path.ends_with(file)).unwrap();
        assert_eq!(game.platform_id, system, "{file}");
    }
    // Pass B hashed the inner `.a26`; the runner's read gives the same bytes.
    let pitfall = games
        .iter()
        .find(|g| g.rom_path.ends_with("pitfall.zip"))
        .unwrap();
    assert_eq!(pitfall.sha1, hash_rom_bytes(&vcs, &ATARI2600).sha1);
    let rom = read_rom_archive(Path::new(&pitfall.rom_path)).unwrap();
    assert_eq!((rom.system.id, rom.bytes), ("atari2600", vcs));
    // A zipped unknown `.bin` behaves as before M17 (Mega Drive probe problem).
    let odd: Vec<&str> = summary
        .failures
        .iter()
        .filter(|f| f.path.ends_with("odd.zip"))
        .map(|f| f.reason.as_str())
        .collect();
    assert_eq!(odd.len(), 1, "{:?}", summary.failures);
    assert!(odd[0].contains("missing Mega Drive header"), "{}", odd[0]);
}
