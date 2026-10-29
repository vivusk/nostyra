//! M10 exit criterion: one library mixing SNES / GB / Genesis `.bin` / PS1 with the
//! N64 test ROM in all three byte orders (`.z64`, `.v64`, `.n64`) and a junk `.z64`.
//! The three N64 copies become three `n64` rows sharing the `.z64` content id (one
//! save folder), the junk `.z64` is reported, and the other systems are unchanged.
#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use disc_image::test_images::{ps1_sectors, to_raw, US_LICENCE};
use game_library::{game_content_sha1, hash_rom_bytes, scan_library, SystemCatalogs};
use storage_db::{list_games, GameListQuery};
use system_catalog::{n64_to_order, N64ByteOrder, NINTENDO_64};

fn test_rom(file: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../test-assets/roms")
            .join(file),
    )
    .unwrap()
}

fn name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn n64_byte_orders_share_one_row_identity_beside_other_systems() {
    let lib = TempLibrary::new("n64-mixed");
    let z64 = test_rom("nostyra-test.z64");
    lib.write("SNES/a.sfc", &test_rom("nostyra-test.sfc"));
    lib.write("Handheld/b.gb", &test_rom("nostyra-test.gb"));
    lib.write("Mixed/genesis.bin", &test_rom("nostyra-test.gen"));
    lib.write(
        "PS1/game.bin",
        &to_raw(
            &ps1_sectors(Some(US_LICENCE), r"BOOT = cdrom:\SLUS_000.01;1"),
            2,
        ),
    );
    lib.write("N64/nostyra-test.z64", &z64);
    lib.write(
        "N64/nostyra-test.v64",
        &n64_to_order(&z64, N64ByteOrder::ByteSwapped),
    );
    let n64_copy = lib.write(
        "Mixed/nostyra-test.n64",
        &n64_to_order(&z64, N64ByteOrder::LittleEndian),
    );
    lib.write("N64/junk.z64", &vec![0x55u8; 0x4000]);

    let summary = scan_library(
        &lib.db,
        std::slice::from_ref(&lib.folder),
        &SystemCatalogs::new(None),
        10,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(summary.failures.len(), 1, "{:?}", summary.failures);
    assert!(summary.failures[0].path.ends_with("junk.z64"));
    assert!(summary.failures[0].reason.contains("missing N64 header"));

    let games = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    let listed: BTreeMap<String, String> = games
        .iter()
        .map(|game| (name(&game.rom_path), game.platform_id.clone()))
        .collect();
    let expected: BTreeMap<String, String> = [
        ("a.sfc", "snes"),
        ("b.gb", "gb"),
        ("genesis.bin", "genesis"),
        ("game.bin", "psx"),
        ("nostyra-test.z64", "n64"),
        ("nostyra-test.v64", "n64"),
        ("nostyra-test.n64", "n64"),
    ]
    .into_iter()
    .map(|(file, system)| (file.to_string(), system.to_string()))
    .collect();
    assert_eq!(listed, expected);

    let content_id = hash_rom_bytes(&z64, &NINTENDO_64).sha1;
    let n64_rows: Vec<&str> = games
        .iter()
        .filter(|game| game.platform_id == NINTENDO_64.id)
        .map(|game| game.sha1.as_str())
        .collect();
    assert_eq!(n64_rows, [content_id.as_str(); 3]);
    assert_eq!(
        game_content_sha1(&n64_copy, &NINTENDO_64).unwrap(),
        content_id
    );
}
