//! PlayStation identification end to end: first-track SHA1 → DAT title, serial-only
//! → file title + DAT metadata, `.pbp` by serial, relink of a moved cue game, rescan
//! fingerprint over track files, rematch by stored serial.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use disc_image::test_images::{pbp_image, ps1_sectors, to_raw, US_LICENCE};
use game_library::{
    hash_file_streaming, scan_library, DatEntry, DatIndex, MetadataIndex, ScanSummary,
    SystemCatalog, SystemCatalogs,
};
use storage_db::{list_games, Game, GameListQuery, GameSort};
use system_catalog::PSX;

fn disc(exe: &str) -> Vec<u8> {
    to_raw(
        &ps1_sectors(Some(US_LICENCE), &format!(r"BOOT = cdrom:\{exe};1")),
        2,
    )
}

fn entry(name: &str, serial: &str, size: u64, crc: &str, sha1: Option<&str>) -> DatEntry {
    DatEntry {
        name: name.into(),
        region: None,
        size,
        crc32: crc.into(),
        sha1: sha1.map(str::to_string),
        serial: Some(serial.into()),
        metadata_crc32: crc.into(),
    }
}

/// Serial-keyed metadat like libretro-database's PS1 developer file (all attributes
/// in one file, ROMs without CRC), plus header-only files for the other four.
fn ps1_metadata(dir: &Path) -> MetadataIndex {
    let dev = "clrmamepro ( name \"Sony - PlayStation\" )\ngame (\n\tname \"Fake Racer (USA)\"\n\tserial \"SLUS-01234\"\n\tdeveloper \"Studio X\"\n\tgenre \"Racing\"\n\treleaseyear \"1999\"\n\tusers \"2\"\n\trom (\n\t\tserial \"SLUS-01234\"\n\t\tname \"Fake Racer (USA).cue\"\n\t)\n)\n";
    for (folder, text) in [
        ("developer", dev),
        ("publisher", "clrmamepro ( name \"x\" )\n"),
        ("genre", "clrmamepro ( name \"x\" )\n"),
        ("releaseyear", "clrmamepro ( name \"x\" )\n"),
        ("maxusers", "clrmamepro ( name \"x\" )\n"),
    ] {
        std::fs::create_dir_all(dir.join(folder)).unwrap();
        std::fs::write(dir.join(folder).join(PSX.metadat_file), text).unwrap();
    }
    MetadataIndex::load(dir, PSX.metadat_file)
}

fn scan(lib: &TempLibrary, dat: &DatIndex, metadata: &MetadataIndex) -> ScanSummary {
    let catalogs = SystemCatalogs::from_loaded([(
        PSX.id,
        SystemCatalog {
            dat: dat.clone(),
            metadata: metadata.clone(),
        },
    )]);
    scan_library(
        &lib.db,
        std::slice::from_ref(&lib.folder),
        &catalogs,
        10,
        &|_| {},
        &AtomicBool::new(false),
    )
    .unwrap()
}

fn games(lib: &TempLibrary) -> Vec<Game> {
    let query = GameListQuery {
        sort: GameSort::Title,
        ..GameListQuery::default()
    };
    list_games(&lib.db.lock().unwrap(), &query).unwrap()
}

#[test]
fn sha1_match_takes_dat_title_and_serial_match_keeps_file_title() {
    let lib = TempLibrary::new("psx-id");
    let meta_dir = tempfile::tempdir().unwrap();
    let metadata = ps1_metadata(meta_dir.path());
    assert!(metadata.is_complete());
    let exact = lib.write("Exact (Track 1).bin", &disc("SCUS_944.26"));
    let cue = "FILE \"Exact (Track 1).bin\" BINARY\n TRACK 01 MODE2/2352\n  INDEX 01 00:00:00\n";
    lib.write("Exact.cue", cue.as_bytes());
    lib.write("Fake Racer VN Translation.bin", &disc("SLUS_012.34"));
    lib.write("Classic.pbp", &pbp_image("ME", "SLUS00594"));
    let track = hash_file_streaming(&exact).unwrap();
    let dat = DatIndex::from_entries(vec![
        entry(
            "Exact Game (USA)",
            "SCUS-94426",
            track.size,
            &track.crc32,
            Some(&track.sha1),
        ),
        // Duplicate serial: the size tie-break picks the second (same size as ours).
        entry(
            "Fake Racer (Japan)",
            "SLUS-01234",
            1,
            "00000001",
            Some("00"),
        ),
        entry(
            "Fake Racer (USA)",
            "SLUS-01234",
            disc("X").len() as u64,
            "00000002",
            Some("01"),
        ),
        entry("Old Classic (USA)", "SLUS-00594", 9, "00000003", Some("02")),
    ]);
    let summary = scan(&lib, &dat, &metadata);
    assert_eq!(
        (summary.added, summary.matched),
        (3, 3),
        "{:?}",
        summary.failures
    );
    let rows = games(&lib);
    let titles: Vec<(&str, Option<&str>, Option<&str>)> = rows
        .iter()
        .map(|g| (g.title.as_str(), g.dat_name.as_deref(), g.serial.as_deref()))
        .collect();
    assert_eq!(
        titles,
        [
            ("Classic", Some("Old Classic (USA)"), Some("SLUS-00594")),
            ("Exact Game", Some("Exact Game (USA)"), Some("SCUS-94426")),
            (
                "Fake Racer VN Translation",
                Some("Fake Racer (USA)"),
                Some("SLUS-01234")
            ),
        ]
    );
    let racer = rows.iter().find(|g| g.title.starts_with("Fake")).unwrap();
    assert_eq!(racer.developer.as_deref(), Some("Studio X"));
    assert_eq!(racer.genre.as_deref(), Some("Racing"));
    assert_eq!(rows[1].sha1, track.sha1, "cue game stores its track-1 hash");
}

#[test]
fn moved_cue_game_relinks_and_track_changes_refresh_the_row() {
    let lib = TempLibrary::new("psx-relink");
    let cue = "FILE \"G.bin\" BINARY\n TRACK 01 MODE2/2352\n  INDEX 01 00:00:00\nFILE \"G2.bin\" BINARY\n TRACK 02 AUDIO\n  INDEX 01 00:00:00\n";
    lib.write("G.bin", &disc("SLUS_012.34"));
    lib.write("G2.bin", &[1u8; 2352 * 4]);
    lib.write("G.cue", cue.as_bytes());
    let empty = MetadataIndex::empty();
    assert_eq!(scan(&lib, &DatIndex::empty(), &empty).added, 1);
    // An audio track changes size → fingerprint differs → re-hashed (same identity).
    lib.write("G2.bin", &[1u8; 2352 * 5]);
    let again = scan(&lib, &DatIndex::empty(), &empty);
    assert_eq!((again.updated, again.unchanged), (1, 0));
    // Whole game moved into a subfolder: same row by track-1 SHA1.
    std::fs::create_dir_all(lib.roms().join("ps1")).unwrap();
    for name in ["G.bin", "G2.bin", "G.cue"] {
        std::fs::rename(lib.roms().join(name), lib.roms().join("ps1").join(name)).unwrap();
    }
    let moved = scan(&lib, &DatIndex::empty(), &empty);
    assert_eq!((moved.relinked, moved.added), (1, 0), "{moved:?}");
    assert_eq!(games(&lib).len(), 1);
}

#[test]
fn unchanged_disc_rematches_by_stored_serial() {
    let lib = TempLibrary::new("psx-rematch");
    lib.write("Hack.bin", &disc("SLUS_012.34"));
    let empty = MetadataIndex::empty();
    scan(&lib, &DatIndex::empty(), &empty);
    assert!(games(&lib)[0].needs_review());
    assert_eq!(games(&lib)[0].serial.as_deref(), Some("SLUS-01234"));
    let dat = DatIndex::from_entries(vec![entry("Fake Racer (USA)", "SLUS-01234", 1, "0", None)]);
    let summary = scan(&lib, &dat, &empty);
    assert_eq!((summary.unchanged, summary.rematched), (1, 1));
    let game = &games(&lib)[0];
    assert_eq!(
        (game.title.as_str(), game.dat_name.as_deref()),
        ("Hack", Some("Fake Racer (USA)"))
    );
}
