//! PlayStation 2 identification end to end: every container of one disc scans to
//! one row identity (serial + canonical content id, no disc bytes hashed), the
//! Redump DAT matched by the 10-char serial key (variant block too), the developer
//! metadat joined, a serial-less disc keyed by volume + boot path, scan == runner
//! content id. Vendored Redump PS2 data: `ps2_vendored_data_tests`.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use disc_image::test_images::{to_iso, to_raw};
use disc_image::test_ps2_images::{ps2_sectors, ps2_volume, zso, TEST_EXE, TEST_SERIAL};
use disc_image::test_psp_containers::{cso, dvd_chd};
use game_library::{
    game_content_sha1, hash_bytes, scan_library, DatEntry, DatIndex, MetadataIndex, ScanSummary,
    SystemCatalog, SystemCatalogs,
};
use storage_db::{list_games, Game, GameListQuery};
use system_catalog::{system, DiscSignature, PS2};

fn entry(name: &str, serial: &str, size: u64) -> DatEntry {
    DatEntry {
        name: name.into(),
        region: None,
        size,
        crc32: "00000000".into(),
        sha1: Some("00".into()),
        serial: Some(serial.into()),
        metadata_crc32: "00000000".into(),
    }
}

/// Serial-keyed developer metadat like libretro-database's PS2 file.
fn ps2_metadata(dir: &Path) -> MetadataIndex {
    let dev = format!(
        "clrmamepro ( name \"Sony - PlayStation 2\" )\ngame (\n\tname \"Nostyra Test PS2 (World)\"\n\tserial \"{TEST_SERIAL}\"\n\tdeveloper \"Studio N\"\n\tgenre \"Action\"\n\treleaseyear \"2026\"\n\tusers \"2\"\n\trom (\n\t\tserial \"{TEST_SERIAL}\"\n\t\tname \"Nostyra Test PS2 (World).cue\"\n\t)\n)\n"
    );
    for (folder, text) in [
        ("developer", dev.as_str()),
        ("publisher", "clrmamepro ( name \"x\" )\n"),
        ("genre", "clrmamepro ( name \"x\" )\n"),
        ("releaseyear", "clrmamepro ( name \"x\" )\n"),
        ("maxusers", "clrmamepro ( name \"x\" )\n"),
    ] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
        std::fs::write(dir.join(folder).join(PS2.metadat_file), text).unwrap();
    }
    MetadataIndex::load(dir, PS2.metadat_file).with_serial_rule(Some(DiscSignature::PlayStation2))
}

fn scan(lib: &TempLibrary, dat: DatIndex, metadata: MetadataIndex) -> (ScanSummary, Vec<Game>) {
    let catalogs = SystemCatalogs::from_loaded([(
        PS2.id,
        SystemCatalog {
            dat: dat.with_serial_rule(Some(DiscSignature::PlayStation2)),
            metadata,
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
    let rows = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    (summary, rows)
}

fn name_of(game: &Game) -> String {
    Path::new(&game.rom_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn every_container_shares_one_serial_identity_and_matches_the_dat_by_serial_key() {
    let lib = TempLibrary::new("ps2-id");
    let meta_dir = tempfile::tempdir().unwrap();
    let metadata = ps2_metadata(meta_dir.path());
    assert!(metadata.is_complete());
    let sectors = ps2_sectors(TEST_EXE);
    let iso = to_iso(&sectors);
    let raw = to_raw(&sectors, 2);
    lib.write("Game.iso", &iso);
    lib.write("Cd/Game (Track 1).bin", &raw);
    let cue = "FILE \"Game (Track 1).bin\" BINARY\n TRACK 01 MODE2/2352\n  INDEX 01 00:00:00\n";
    lib.write("Cd/Game.cue", cue.as_bytes());
    lib.write("Game.chd", &dvd_chd(&iso, true));
    lib.write("Game.cso", &cso(&iso, 2048, 0));
    lib.write("Game.zso", &zso(&iso, 2048, 1));
    // A playlist hides its member discs; the row is the `.m3u` (first disc identity).
    lib.write("Set/Disc1.iso", &iso);
    lib.write("Set/Disc2.chd", &dvd_chd(&iso, false));
    lib.write(
        "Set.m3u",
        b"Set/Disc1.iso
Set/Disc2.chd
",
    );
    // The disc side has `NSTR-00001`; the DAT lists two Redump-style blocks (the
    // variant one first) and the same-size entry wins the tie-break.
    let dat = DatIndex::from_entries(vec![
        entry(
            "Nostyra Test PS2 (World) (Alt)",
            &format!("{TEST_SERIAL}-0"),
            1,
        ),
        entry("Nostyra Test PS2 (World)", TEST_SERIAL, iso.len() as u64),
    ]);
    let (summary, rows) = scan(&lib, dat, metadata);
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    assert_eq!((summary.added, summary.matched), (6, 6));
    let want = hash_bytes(format!("ps2:{TEST_SERIAL}").as_bytes(), 0).sha1;
    for game in &rows {
        let name = name_of(game);
        assert_eq!(game.platform_id, "ps2", "{name}");
        assert_eq!(game.sha1, want, "{name}");
        assert_eq!(game.serial.as_deref(), Some(TEST_SERIAL), "{name}");
        assert!(
            game.dat_name
                .as_deref()
                .unwrap()
                .starts_with("Nostyra Test PS2 (World)"),
            "{name}"
        );
        assert_eq!(game.developer.as_deref(), Some("Studio N"), "{name}");
        assert_eq!(game.genre.as_deref(), Some("Action"), "{name}");
        // Serial match keeps the file's own title.
        assert_eq!(game.title, name.rsplit_once('.').unwrap().0, "{name}");
        let spec = system(&game.platform_id).unwrap();
        let runner_id = game_content_sha1(Path::new(&game.rom_path), spec).unwrap();
        assert_eq!(runner_id, game.sha1, "{name}");
    }
    let by_name = |name: &str| rows.iter().find(|g| name_of(g) == name).unwrap();
    assert_eq!(
        by_name("Game.iso").dat_name.as_deref(),
        Some("Nostyra Test PS2 (World)"),
        "size tie-break picks the same-size block"
    );
    assert_eq!(by_name("Game.iso").file_size, iso.len() as i64);
}

#[test]
fn serial_less_discs_are_ps2_rows_keyed_by_volume_and_boot_path() {
    let lib = TempLibrary::new("ps2-hb");
    let cnf = b"BOOT2 = cdrom0:\\BOOT.ELF;1\r\nVER = 1.00\r\nVMODE = PAL\r\n";
    for volume in ["NOSTYRA_HB", "OTHER_HB"] {
        let sectors = ps2_volume(volume, &[("SYSTEM.CNF", cnf), ("BOOT.ELF", &[7; 48])]);
        lib.write(&format!("{volume}.iso"), &to_iso(&sectors));
    }
    let dat = DatIndex::from_entries(vec![entry("Some Game (USA)", "SLUS-20001", 1)]);
    let (summary, rows) = scan(&lib, dat, MetadataIndex::empty());
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    assert_eq!((summary.added, summary.matched), (2, 0));
    let id = |volume: &str| {
        let game = rows
            .iter()
            .find(|g| name_of(g) == format!("{volume}.iso"))
            .unwrap();
        assert_eq!(
            (game.platform_id.as_str(), game.serial.as_deref()),
            ("ps2", None)
        );
        assert_eq!(
            (game.dat_name.as_deref(), game.title.as_str()),
            (None, volume)
        );
        game.sha1.clone()
    };
    assert_eq!(
        id("NOSTYRA_HB"),
        hash_bytes(b"ps2:vol:NOSTYRA_HB/BOOT.ELF;1", 0).sha1
    );
    assert_ne!(id("NOSTYRA_HB"), id("OTHER_HB"));
}
