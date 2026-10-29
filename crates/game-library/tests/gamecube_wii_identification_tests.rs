//! GameCube / Wii identification end to end: every container classified by its
//! header magic, identity = the game ID (serial) and the canonical-ID content id
//! `SHA1("<sys>:<ID6>")` — shared by ISO / RVZ copies, disc 1 / disc 2, revisions and
//! the `.m3u` — matched to the DAT by the 4-char game code; `.dol` = file hash;
//! scan == runner content id; a 4 GiB image is identified from its header alone.
//! Vendored Redump data and rejected files: `gamecube_wii_vendored_data_tests`.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use disc_image::test_nintendo_images::{ciso, MTSPR_HID4_R0};
use disc_image::test_nintendo_images::{disc_header, dol, gcz, plain, wbfs, wia, BLR};
use disc_image::NintendoDiscKind::{GameCube, Wii};
use game_library::{
    game_content_sha1, hash_bytes, hash_file_streaming, scan_library, DatEntry, DatIndex,
    MetadataIndex, ScanSummary, SystemCatalog, SystemCatalogs,
};
use storage_db::{list_games, Game, GameListQuery};
use system_catalog::{system, DiscSignature, SystemSpec, GC, WII};

fn entry(name: &str, serial: &str) -> DatEntry {
    DatEntry {
        name: name.into(),
        region: None,
        size: 1_459_978_240,
        crc32: "00000000".into(),
        sha1: Some("00".into()),
        serial: Some(serial.into()),
        metadata_crc32: "00000000".into(),
    }
}

fn catalog(spec: &'static SystemSpec, entries: Vec<DatEntry>) -> (&'static str, SystemCatalog) {
    let rule = Some(if spec.id == WII.id {
        DiscSignature::Wii
    } else {
        DiscSignature::GameCube
    });
    let catalog = SystemCatalog {
        dat: DatIndex::from_entries(entries).with_serial_rule(rule),
        metadata: MetadataIndex::empty().with_serial_rule(rule),
    };
    (spec.id, catalog)
}

fn scan(lib: &TempLibrary) -> (ScanSummary, Vec<Game>) {
    let catalogs = SystemCatalogs::from_loaded([
        catalog(
            &GC,
            vec![entry("Super Smash Bros. Melee (USA)", "DL-DOL-GALE-USA")],
        ),
        catalog(&WII, vec![entry("Wii Sports (USA)", "RVL-RSPE-USA")]),
    ]);
    let folders = std::slice::from_ref(&lib.folder);
    let summary = scan_library(
        &lib.db,
        folders,
        &catalogs,
        10,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    let rows = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    (summary, rows)
}

fn canonical(text: &str) -> String {
    hash_bytes(text.as_bytes(), 0).sha1
}

fn row<'a>(rows: &'a [Game], file: &str) -> &'a Game {
    rows.iter()
        .find(|g| g.rom_path.replace('\\', "/").ends_with(file))
        .unwrap_or_else(|| panic!("{file} not listed"))
}

#[test]
fn every_container_disc_and_revision_shares_the_game_id_content_id() {
    let lib = TempLibrary::new("gcwii-id");
    let gc = |disc, rev| disc_header(GameCube, "GALE01", disc, rev);
    let wii = |rev| disc_header(Wii, "RSPE01", 0, rev);
    lib.write("Melee.iso", &plain(&gc(0, 0), 0x8000));
    lib.write("Melee Rev 1.gcm", &plain(&gc(0, 1), 0x8000));
    lib.write("Melee.rvz", &wia(&gc(0, 0), true));
    lib.write("Melee.ciso", &ciso(&gc(0, 0), true));
    lib.write("Melee.gcz", &gcz(&gc(0, 0), 2, false));
    lib.write("Set/Disc 1.iso", &plain(&gc(0, 0), 0x8000));
    lib.write("Set/Disc 2.rvz", &wia(&gc(1, 0), true));
    lib.write("Set/Set.m3u", b"Disc 1.iso\nDisc 2.rvz\n");
    lib.write("Loose Disc 2.iso", &plain(&gc(1, 0), 0x8000));
    lib.write("Sports.wbfs", &wbfs(&wii(0), 9));
    lib.write("Sports Rev 1.wia", &wia(&wii(1), false));
    lib.write("Sports.iso", &plain(&wii(0), 0x8000));
    let gc_dol = lib.write("Homebrew/gc.dol", &dol(&[BLR], &[]));
    let wii_dol = lib.write("Homebrew/wii.dol", &dol(&[MTSPR_HID4_R0, BLR], &[]));
    let (summary, rows) = scan(&lib);
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    assert_eq!(rows.len(), 12, "Set/ = one m3u row");

    let melee = canonical("gc:GALE01");
    for file in [
        "/Melee.iso",
        "/Melee Rev 1.gcm",
        "/Melee.rvz",
        "/Melee.ciso",
        "/Melee.gcz",
        "/Set.m3u",
        "/Loose Disc 2.iso",
    ] {
        let game = row(&rows, file);
        assert_eq!(game.platform_id, "gc", "{file}");
        assert_eq!(game.sha1, melee, "{file}");
        assert_eq!(game.serial.as_deref(), Some("GALE01"), "{file}");
        assert_eq!(
            game.dat_name.as_deref(),
            Some("Super Smash Bros. Melee (USA)")
        );
        // Serial match is "probable": the title stays the file name.
        assert_ne!(game.title, "Super Smash Bros. Melee");
    }
    let sports = canonical("wii:RSPE01");
    assert_ne!(sports, melee);
    for file in ["/Sports.wbfs", "/Sports Rev 1.wia", "/Sports.iso"] {
        let game = row(&rows, file);
        assert_eq!(
            (game.platform_id.as_str(), game.sha1.as_str()),
            ("wii", sports.as_str())
        );
        assert_eq!(game.dat_name.as_deref(), Some("Wii Sports (USA)"), "{file}");
    }
    for (path, platform) in [(&gc_dol, "gc"), (&wii_dol, "wii")] {
        let game = row(
            &rows,
            &format!("/{}", path.file_name().unwrap().to_str().unwrap()),
        );
        assert_eq!(game.platform_id, platform);
        assert_eq!(game.sha1, hash_file_streaming(path).unwrap().sha1);
        assert_eq!(
            (game.serial.as_deref(), game.dat_name.as_deref()),
            (None, None)
        );
    }
    // The runner's fallback content id (no `--content-sha1`) is the scan value.
    for game in &rows {
        let spec = system(&game.platform_id).unwrap();
        let path = std::path::Path::new(&game.rom_path);
        assert_eq!(
            game_content_sha1(path, spec).unwrap(),
            game.sha1,
            "{}",
            game.rom_path
        );
    }
}

#[test]
fn a_4_gib_image_is_identified_from_its_header_alone() {
    let lib = TempLibrary::new("gcwii-big");
    let path = lib.write("Big.iso", &plain(&disc_header(Wii, "RSPE01", 0, 0), 0x400));
    // A hole after the header: reading the whole file would take seconds.
    let size = 4 * 1024 * 1024 * 1024 + 0x8000;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(size)
        .unwrap();
    let (summary, rows) = scan(&lib);
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    let game = row(&rows, "/Big.iso");
    assert_eq!(game.platform_id, "wii");
    // A canonical-ID hash, which the 4 GiB file's own SHA1 cannot be.
    assert_eq!(game.sha1, canonical("wii:RSPE01"));
    assert_eq!(game_content_sha1(&path, &WII).unwrap(), game.sha1);
    // The file size still reaches the row (DAT size tie-breaks see the file).
    assert_eq!(game.file_size, i64::try_from(size).unwrap());
}
