//! Nintendo DS identification: whole-file hash against a fixture DAT laid out like
//! libretro-database's (game-level `serial` = game code), game-code fallback for
//! dumps the DAT has no hash for (title stays the file name), the code stored and
//! used to rematch, large files hashed streaming and still matched.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use game_library::{
    hash_rom_bytes, scan_library, DatIndex, ScanSummary, SystemCatalog, SystemCatalogs,
};
use storage_db::{list_games, Game, GameListQuery};
use system_catalog::NDS;

/// CRC-16/MODBUS: the DS header CRC.
fn crc16(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0xFFFF, |crc, &b| {
        (0..8).fold(crc ^ u16::from(b), |c, _| {
            if c & 1 != 0 {
                (c >> 1) ^ 0xA001
            } else {
                c >> 1
            }
        })
    })
}

/// `size` bytes with a valid DS header: game code `code`, ARM9 at 0x200, ARM7 at
/// 0x400; the body depends on `seed`.
fn nds_rom(size: usize, code: &[u8; 4], seed: u8) -> Vec<u8> {
    let mut rom: Vec<u8> = (0..size).map(|i| (i % 251) as u8 ^ seed).collect();
    rom[..0x200].fill(0);
    rom[0x0C..0x10].copy_from_slice(code);
    rom[0x20..0x24].copy_from_slice(&0x200u32.to_le_bytes());
    rom[0x24..0x28].copy_from_slice(&0x0200_0000u32.to_le_bytes());
    rom[0x30..0x34].copy_from_slice(&0x400u32.to_le_bytes());
    let crc = crc16(&rom[..0x15E]);
    rom[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
    rom
}

/// DAT game line as libretro-database writes it for the DS.
fn dat_game(name: &str, serial: &str, rom: &[u8]) -> String {
    let h = hash_rom_bytes(rom, &NDS);
    format!(
        "game (\n\tname \"{name}\"\n\tserial \"{serial}\"\n\trom ( name \"{name}.nds\" size {} crc {} sha1 {} serial \"{serial}\" )\n)\n",
        h.size, h.crc32, h.sha1
    )
}

fn catalogs(dir: &Path, games: &[String]) -> SystemCatalogs {
    let path = dir.join(NDS.dat_file);
    let text = format!(
        "clrmamepro ( name \"Nintendo - Nintendo DS\" )\n{}",
        games.concat()
    );
    std::fs::write(&path, text).unwrap();
    let dat = DatIndex::load(&path).unwrap();
    SystemCatalogs::from_loaded([(
        "nds",
        SystemCatalog {
            dat,
            ..SystemCatalog::default()
        },
    )])
}

fn scan(lib: &TempLibrary, catalogs: &SystemCatalogs) -> ScanSummary {
    let folders = std::slice::from_ref(&lib.folder);
    scan_library(
        &lib.db,
        folders,
        catalogs,
        10,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap()
}

fn game(lib: &TempLibrary, file: &str) -> Game {
    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    games
        .into_iter()
        .find(|g| g.rom_path.ends_with(file))
        .unwrap_or_else(|| panic!("{file} listed"))
}

#[test]
fn dat_hash_then_game_code_identify_ds_dumps() {
    let db = tempfile::Builder::new()
        .prefix("nostyra-gl-nds-db-")
        .tempdir()
        .unwrap();
    let kart = nds_rom(0x4000, b"AMCE", 1);
    let other = nds_rom(0x8000, b"AMCE", 2);
    let cats = catalogs(
        db.path(),
        &[
            dat_game("Fake Kart DS (USA)", "AMCE", &kart),
            dat_game("Fake Kart DS (USA) (Rev 1)", "AMCE", &other),
        ],
    );
    let lib = TempLibrary::new("nds-identify");
    lib.write("ds/kart.nds", &kart);
    let mut hack = nds_rom(0x8000, b"AMCE", 3);
    hack[0x2000] ^= 0xFF;
    lib.write("ds/Kart Hack (Translated).nds", &hack);
    lib.write("ds/homebrew.nds", &nds_rom(0x1000, b"####", 4));
    let summary = scan(&lib, &cats);
    assert_eq!(
        (summary.added, summary.matched, summary.unmatched),
        (3, 2, 1)
    );

    let by_hash = game(&lib, "kart.nds");
    assert_eq!(by_hash.platform_id, "nds");
    assert_eq!(by_hash.title, "Fake Kart DS");
    assert_eq!(by_hash.serial.as_deref(), Some("AMCE"));
    let by_code = game(&lib, "Kart Hack (Translated).nds");
    assert_eq!(
        by_code.title, "Kart Hack",
        "probable match keeps the file name"
    );
    assert_eq!(
        by_code.dat_name.as_deref(),
        Some("Fake Kart DS (USA) (Rev 1)"),
        "shared code: the entry of the same size wins"
    );
    assert_eq!(by_code.serial.as_deref(), Some("AMCE"));
    let homebrew = game(&lib, "homebrew.nds");
    assert!(homebrew.needs_review() && homebrew.serial.is_none());
}

#[test]
fn stored_game_code_rematches_after_the_dat_arrives() {
    let db = tempfile::Builder::new()
        .prefix("nostyra-gl-nds-db-")
        .tempdir()
        .unwrap();
    let lib = TempLibrary::new("nds-rematch");
    lib.write("Puzzle Hack.nds", &nds_rom(0x2000, b"ZZZP", 5));
    scan(&lib, &catalogs(db.path(), &[]));
    let before = game(&lib, "Puzzle Hack.nds");
    assert!(before.needs_review());
    assert_eq!(before.serial.as_deref(), Some("ZZZP"));
    let dat = [dat_game(
        "Fake Puzzle (Europe)",
        "ZZZP",
        &nds_rom(0x4000, b"ZZZP", 6),
    )];
    let summary = scan(&lib, &catalogs(db.path(), &dat));
    assert_eq!((summary.unchanged, summary.rematched), (1, 1));
    let after = game(&lib, "Puzzle Hack.nds");
    assert_eq!(
        (after.title.as_str(), after.dat_name.as_deref()),
        ("Puzzle Hack", Some("Fake Puzzle (Europe)"))
    );
}

#[test]
fn large_ds_dumps_are_streamed_and_matched_by_hash_or_code() {
    let db = tempfile::Builder::new()
        .prefix("nostyra-gl-nds-db-")
        .tempdir()
        .unwrap();
    // Just over the 32 MiB streaming threshold: the hasher returns no bytes, the
    // game code comes from its own read.
    let mut big = nds_rom(32 * 1024 * 1024 + 0x1000, b"BIGE", 7);
    let cats = catalogs(db.path(), &[dat_game("Fake Big (USA)", "BIGE", &big)]);
    let lib = TempLibrary::new("nds-stream");
    lib.write("big.nds", &big);
    big[0x10_0000] ^= 0xFF;
    lib.write("Big Hack.nds", &big);
    let summary = scan(&lib, &cats);
    assert_eq!((summary.matched, summary.unmatched), (2, 0));
    assert_eq!(game(&lib, "big.nds").title, "Fake Big");
    let code_only = game(&lib, "Big Hack.nds");
    assert_eq!(code_only.title, "Big Hack");
    assert_eq!(code_only.dat_name.as_deref(), Some("Fake Big (USA)"));
}

#[test]
fn vendored_ds_dat_finds_games_by_game_code() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    let catalog = catalogs.get(NDS.id);
    assert!(catalog.dat.len() > 7_000, "DS DAT entries");
    assert!(catalog.metadata.is_complete(), "DS metadat");
    let kart = catalog.dat.lookup_serial("AMCE", Some(32 * 1024 * 1024));
    assert_eq!(
        kart.map(|e| e.name.as_str()),
        Some("Mario Kart DS (USA, Australia) (En,Fr,De,Es,It)")
    );
}
