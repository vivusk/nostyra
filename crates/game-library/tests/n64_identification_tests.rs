//! Nintendo 64 identification: `.z64`, `.v64` and `.n64` copies of one dump hash as
//! the big-endian `.z64` image, so all three match the DAT's `.z64` row with its
//! metadata and share one content id (one save folder); a junk `.z64` is reported.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use game_library::{game_content_sha1, hash_rom_bytes, scan_library, SystemCatalogs};
use storage_db::{get_game_metadata, list_games, GameListQuery};
use system_catalog::{n64_to_order, N64ByteOrder, NINTENDO_64};

const GAME: &str = "Fake Plumber 64 (USA)";

/// 64 KiB big-endian (`.z64`) image.
fn z64_rom() -> Vec<u8> {
    let mut rom: Vec<u8> = (0..0x10000u32).map(|i| (i * 19 % 251) as u8).collect();
    rom[..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
    rom
}

/// DAT shaped like the No-Intro N64 one: the game listed twice, as a `.z64` row and
/// a `.v64` row (hash of the swapped file), + a genre metadat keyed by both CRCs.
fn write_database(dir: &Path, z64: &[u8]) {
    let z = hash_rom_bytes(z64, &NINTENDO_64);
    let v64 = n64_to_order(z64, N64ByteOrder::ByteSwapped);
    let v = (crc32fast::hash(&v64), sha1_hex(&v64));
    let dat = format!(
        "clrmamepro ( name \"Nintendo - Nintendo 64\" )\n\
         game (\n\tname \"{GAME}\"\n\trom ( name \"{GAME}.z64\" size {} crc {} sha1 {} )\n)\n\
         game (\n\tname \"{GAME}\"\n\trom ( name \"{GAME}.v64\" size {} crc {:08X} sha1 {} )\n)\n",
        z.size,
        z.crc32,
        z.sha1,
        v64.len(),
        v.0,
        v.1
    );
    std::fs::write(dir.join(NINTENDO_64.dat_file), dat).unwrap();
    let genre = format!(
        "game (\n\tcomment \"{GAME}\"\n\tgenre \"Platform\"\n\trom ( crc {:08X} )\n)\n\
         game (\n\tcomment \"{GAME}\"\n\tgenre \"Platform\"\n\trom ( crc {} )\n)\n",
        v.0, z.crc32
    );
    std::fs::create_dir_all(dir.join("metadat/genre")).unwrap();
    std::fs::write(
        dir.join("metadat/genre").join(NINTENDO_64.metadat_file),
        genre,
    )
    .unwrap();
}

fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest;
    sha1::Sha1::digest(bytes)
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect()
}

#[test]
fn every_byte_order_matches_the_z64_entry_with_metadata() {
    let db = tempfile::Builder::new()
        .prefix("nostyra-gl-n64-db-")
        .tempdir()
        .unwrap();
    let z64 = z64_rom();
    write_database(db.path(), &z64);

    let lib = TempLibrary::new("n64-identify");
    lib.write("N64/plumber.z64", &z64);
    lib.write(
        "N64/plumber.v64",
        &n64_to_order(&z64, N64ByteOrder::ByteSwapped),
    );
    lib.write(
        "Mixed/plumber.N64",
        &n64_to_order(&z64, N64ByteOrder::LittleEndian),
    );
    lib.write("N64/junk.z64", &vec![0x55u8; 0x4000]);

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
    assert_eq!(
        (summary.added, summary.matched, summary.unmatched),
        (3, 3, 0)
    );
    assert_eq!(summary.failures.len(), 1, "{:?}", summary.failures);
    assert!(summary.failures[0].path.ends_with("junk.z64"));
    assert!(summary.failures[0].reason.contains("missing N64 header"));

    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    let z64_sha1 = hash_rom_bytes(&z64, &NINTENDO_64).sha1;
    for file in ["plumber.z64", "plumber.v64", "plumber.N64"] {
        let game = games
            .iter()
            .find(|g| g.rom_path.ends_with(file))
            .unwrap_or_else(|| panic!("{file} listed"));
        assert_eq!(game.platform_id, NINTENDO_64.id, "{file}");
        assert_eq!(game.dat_name.as_deref(), Some(GAME), "{file}");
        assert_eq!(game.sha1, z64_sha1, "{file}");
        let metadata = get_game_metadata(&lib.db.lock().unwrap(), game.id).unwrap();
        assert_eq!(
            metadata.unwrap().genre.as_deref(),
            Some("Platform"),
            "{file}"
        );
    }
}

#[test]
fn every_byte_order_shares_the_z64_content_id() {
    let lib = TempLibrary::new("n64-content-id");
    let z64 = z64_rom();
    let expected = hash_rom_bytes(&z64, &NINTENDO_64).sha1;
    for (name, order) in [
        ("a.z64", N64ByteOrder::BigEndian),
        ("b.v64", N64ByteOrder::ByteSwapped),
        ("c.n64", N64ByteOrder::LittleEndian),
    ] {
        let path = lib.write(name, &n64_to_order(&z64, order));
        assert_eq!(
            game_content_sha1(&path, &NINTENDO_64).unwrap(),
            expected,
            "{name}"
        );
    }
}

#[test]
fn vendored_n64_dat_parses_with_complete_metadata() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    let catalog = catalogs.get(NINTENDO_64.id);
    assert!(catalog.dat.len() > 1000, "n64 DAT entries");
    assert!(catalog.metadata.is_complete(), "n64 metadat");
    assert!(catalogs.loaded_complete());
}
