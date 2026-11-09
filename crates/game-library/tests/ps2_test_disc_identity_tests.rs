//! The committed PlayStation 2 test disc (`test-assets/roms/nostyra-test-ps2.iso`,
//! `build-nostyra-test-ps2.py`): serial `NSTR-00001`, content id
//! `sha1("ps2:NSTR-00001")` shared by its in-test CHD / CSO / ZSO twins, DAT match by
//! serial, scan id == runner id. Synthetic volumes, the serial-less fallback and the
//! ≤ 64 KiB read bound: `ps2_identification_tests` / `rom-identity`.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use disc_image::test_ps2_images::{zso, TEST_SERIAL};
use disc_image::test_psp_containers::{cso, dvd_chd};
use game_library::{
    game_content_sha1, hash_bytes, scan_library, DatEntry, DatIndex, MetadataIndex, SystemCatalog,
    SystemCatalogs,
};
use storage_db::{list_games, GameListQuery};
use system_catalog::{DiscSignature, PS2};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms/nostyra-test-ps2.iso")
}

#[test]
fn the_test_disc_and_its_twins_share_the_serial_identity_and_match_the_dat() {
    let lib = TempLibrary::new("ps2-fixture");
    let iso = std::fs::read(fixture()).unwrap();
    assert_eq!(iso.len(), 2 << 20, "2 MiB padded image (spike D6)");
    lib.write("Nostyra Test PS2.iso", &iso);
    lib.write("Nostyra Test PS2.chd", &dvd_chd(&iso, true));
    lib.write("Nostyra Test PS2.cso", &cso(&iso, 2048, 0));
    lib.write("Nostyra Test PS2.zso", &zso(&iso, 2048, 1));
    let dat = DatIndex::from_entries(vec![DatEntry {
        name: "Nostyra Test PS2 (World)".into(),
        region: None,
        size: iso.len() as u64,
        crc32: "00000000".into(),
        sha1: Some("00".into()),
        serial: Some(TEST_SERIAL.into()),
        metadata_crc32: "00000000".into(),
    }]);
    let catalogs = SystemCatalogs::from_loaded([(
        PS2.id,
        SystemCatalog {
            dat: dat.with_serial_rule(Some(DiscSignature::PlayStation2)),
            metadata: MetadataIndex::empty(),
        },
    )]);
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
    assert_eq!((summary.added, summary.matched), (4, 4));
    let rows = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    let want = hash_bytes(format!("ps2:{TEST_SERIAL}").as_bytes(), 0).sha1;
    for game in &rows {
        assert_eq!(game.platform_id, "ps2", "{}", game.rom_path);
        assert_eq!(game.sha1, want, "{}", game.rom_path);
        assert_eq!(game.serial.as_deref(), Some(TEST_SERIAL));
        assert_eq!(game.dat_name.as_deref(), Some("Nostyra Test PS2 (World)"));
        let runner_id = game_content_sha1(Path::new(&game.rom_path), &PS2).unwrap();
        assert_eq!(runner_id, game.sha1, "{}", game.rom_path);
    }
}

#[test]
fn the_committed_fixture_has_the_serial_content_id() {
    let id = game_content_sha1(&fixture(), &PS2).unwrap();
    assert_eq!(id, hash_bytes(b"ps2:NSTR-00001", 0).sha1);
}
