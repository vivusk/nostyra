//! Atari 2600 / 7800 / Lynx identification: the vendored No-Intro DATs load, `.a26`
//! / `.lyx` / `.o` hash whole, a headered `.a78` / `.lnx` matches the headerless
//! DAT entry (`.bin` / `.lyx`) through the fallback hash while its content id stays
//! the whole file.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use game_library::{game_content_sha1, hash_rom_bytes, scan_library, SystemCatalogs};
use storage_db::{list_games, GameListQuery};
use system_catalog::{SystemSpec, ATARI2600, ATARI7800, LYNX};

fn image(len: u32, seed: u32) -> Vec<u8> {
    (0..len)
        .map(|i| (i.wrapping_mul(seed) % 251) as u8)
        .collect()
}

fn headered(magic_at: usize, magic: &[u8], header_len: usize, data: &[u8]) -> Vec<u8> {
    let mut file = vec![0u8; header_len];
    file[magic_at..magic_at + magic.len()].copy_from_slice(magic);
    file.extend_from_slice(data);
    file
}

/// One DAT per system: `(game, rom file name, bytes the DAT hashes)`.
fn write_dat(dir: &Path, spec: &SystemSpec, entries: &[(&str, &str, &[u8])]) {
    let mut dat = format!("clrmamepro ( name \"{}\" )\n", spec.display_name);
    for (game, rom, bytes) in entries {
        let hash = hash_rom_bytes(bytes, spec);
        dat += &format!(
            "game (\n\tname \"{game}\"\n\trom ( name \"{rom}\" size {} crc {} sha1 {} )\n)\n",
            hash.size, hash.crc32, hash.sha1
        );
    }
    std::fs::write(dir.join(spec.dat_file), dat).unwrap();
}

#[test]
fn atari_dumps_match_their_dat_headered_ones_through_the_fallback() {
    let db = tempfile::Builder::new()
        .prefix("nostyra-gl-atari-db-")
        .tempdir()
        .unwrap();
    let (vcs, prosystem, lynx_raw) = (image(4096, 7), image(16384, 11), image(131_072, 13));
    let mut bs93 = vec![0x80, 0x08, 0x02, 0x00, 0x00, 0x40];
    bs93.extend_from_slice(b"BS93");
    bs93.extend_from_slice(&image(54, 17));
    write_dat(
        db.path(),
        &ATARI2600,
        &[("Fake Pitfall (USA)", "a.a26", &vcs)],
    );
    write_dat(
        db.path(),
        &ATARI7800,
        &[("Fake Asteroids (USA)", "a.bin", &prosystem)],
    );
    write_dat(
        db.path(),
        &LYNX,
        &[
            ("Fake Games (World)", "c.lyx", &lynx_raw),
            ("Fake Homebrew (World)", "h.o", &bs93),
        ],
    );
    let a78 = headered(1, b"ATARI7800", 128, &prosystem);
    let lnx = headered(0, b"LYNX\0", 64, &lynx_raw);

    let lib = TempLibrary::new("atari-identify");
    lib.write("Atari/pitfall.a26", &vcs);
    let a78_path = lib.write("Atari/asteroids.a78", &a78);
    lib.write("Atari/games.lyx", &lynx_raw);
    lib.write("Atari/games.lnx", &lnx);
    lib.write("Atari/homebrew.o", &bs93);
    // GoodTools: headerless dumps named `.a78` / `.lnx` (the No-Intro bytes).
    lib.write("GoodTools/Asteroids (U).a78", &prosystem);
    lib.write("GoodTools/Games (U).lnx", &lynx_raw);

    let catalogs = SystemCatalogs::new(Some(db.path().to_path_buf()));
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
    assert_eq!((summary.added, summary.matched), (7, 7));
    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    for (file, system, name) in [
        ("pitfall.a26", "atari2600", "Fake Pitfall (USA)"),
        ("asteroids.a78", "atari7800", "Fake Asteroids (USA)"),
        ("games.lyx", "lynx", "Fake Games (World)"),
        ("games.lnx", "lynx", "Fake Games (World)"),
        ("homebrew.o", "lynx", "Fake Homebrew (World)"),
        ("Asteroids (U).a78", "atari7800", "Fake Asteroids (USA)"),
        ("Games (U).lnx", "lynx", "Fake Games (World)"),
    ] {
        let game = games.iter().find(|g| g.rom_path.ends_with(file)).unwrap();
        assert_eq!(game.platform_id, system, "{file}");
        assert_eq!(game.dat_name.as_deref(), Some(name), "{file}");
    }
    // Content id (save folder) = the whole headered file, not the `.bin` twin.
    let id = game_content_sha1(&a78_path, &ATARI7800).unwrap();
    assert_eq!(id, hash_rom_bytes(&a78, &ATARI7800).sha1);
    assert_ne!(id, hash_rom_bytes(&prosystem, &ATARI7800).sha1);
}

#[test]
fn vendored_atari_data_loads() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    for (spec, games) in [(&ATARI2600, 890), (&ATARI7800, 337), (&LYNX, 617)] {
        let catalog = catalogs.get(spec.id);
        assert_eq!(catalog.dat.len(), games, "{}", spec.id);
        assert!(catalog.metadata.is_complete(), "{}", spec.id);
    }
}
