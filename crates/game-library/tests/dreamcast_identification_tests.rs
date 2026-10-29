//! Dreamcast identification end to end, in a tree mixed with PlayStation discs:
//! every container classified by content (one row per game, track files hidden),
//! identity = largest data track (Redump SHA1 match) or whole container, serial from
//! IP.BIN matched through the serial rule (`MK-51035` ↔ DAT `51035`), scan == runner
//! content id.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use disc_image::test_cdi_image::{cdi_image, CDI_V3};
use disc_image::test_gd_images::{gd_chd, write_gdi, write_redump_gd_cue};
use disc_image::test_images::{ps1_sectors, to_raw, US_LICENCE};
use game_library::{
    game_content_sha1, hash_file_streaming, scan_library, DatEntry, DatIndex, MetadataIndex,
    SystemCatalog, SystemCatalogs,
};
use storage_db::{list_games, Game, GameListQuery, GameSort};
use system_catalog::{DiscSignature, DREAMCAST, PSX};

fn entry(name: &str, serial: &str, size: u64, sha1: &str) -> DatEntry {
    DatEntry {
        name: name.into(),
        region: None,
        size,
        crc32: "00000000".into(),
        sha1: Some(sha1.into()),
        serial: Some(serial.into()),
        metadata_crc32: "00000000".into(),
    }
}

fn scan(lib: &TempLibrary, dc_dat: DatIndex) -> Vec<Game> {
    let rule = Some(DiscSignature::Dreamcast);
    let catalogs = SystemCatalogs::from_loaded([(
        DREAMCAST.id,
        SystemCatalog {
            dat: dc_dat.with_serial_rule(rule),
            metadata: MetadataIndex::empty().with_serial_rule(rule),
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
    let query = GameListQuery {
        sort: GameSort::Title,
        ..GameListQuery::default()
    };
    list_games(&lib.db.lock().unwrap(), &query).unwrap()
}

#[test]
fn mixed_tree_gives_one_row_per_game_with_ip_bin_serials() {
    let lib = TempLibrary::new("dc-id");
    let roms = lib.roms();
    write_gdi(&roms, "Tosec Game", "NSTY-00001", true);
    let cue = write_redump_gd_cue(&roms, "Redump Game", "NSTY-00002");
    lib.write("Sega Title.chd", &gd_chd("MK-51035"));
    lib.write("Homebrew.cdi", &cdi_image(CDI_V3, "T-1234N"));
    std::fs::create_dir_all(roms.join("multi")).unwrap();
    write_gdi(&roms.join("multi"), "Disc 1", "NSTY-00003", true);
    write_gdi(&roms.join("multi"), "Disc 2", "NSTY-00004", true);
    lib.write("multi/Two Discs.m3u", b"Disc 1.gdi\nDisc 2.gdi\n");
    let ps1_track = to_raw(
        &ps1_sectors(Some(US_LICENCE), r"BOOT = cdrom:\SLUS_012.34;1"),
        2,
    );
    lib.write("Ps1 Game.bin", &ps1_track);
    lib.write(
        "Ps1 Game.cue",
        b"FILE \"Ps1 Game.bin\" BINARY\n TRACK 01 MODE2/2352\n  INDEX 01 00:00:00\n",
    );
    let track3 = hash_file_streaming(&roms.join("Redump Game (Track 3).bin")).unwrap();
    let dat = DatIndex::from_entries(vec![
        entry("Redump Game (USA)", "NSTY-00002", track3.size, &track3.sha1),
        entry("Sega Title (USA)", "51035", 1, "AA"),
    ]);
    let rows = scan(&lib, dat);
    let got: Vec<(&str, &str, Option<&str>, Option<&str>)> = rows
        .iter()
        .map(|g| {
            let name = std::path::Path::new(&g.rom_path)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap();
            (
                name,
                g.platform_id.as_str(),
                g.serial.as_deref(),
                g.dat_name.as_deref(),
            )
        })
        .collect();
    let mut got = got;
    got.sort();
    assert_eq!(
        got,
        [
            ("Homebrew.cdi", "dreamcast", Some("T-1234N"), None),
            ("Ps1 Game.cue", "psx", Some("SLUS-01234"), None),
            (
                "Redump Game.cue",
                "dreamcast",
                Some("NSTY-00002"),
                Some("Redump Game (USA)")
            ),
            (
                "Sega Title.chd",
                "dreamcast",
                Some("MK-51035"),
                Some("Sega Title (USA)")
            ),
            ("Tosec Game.gdi", "dreamcast", Some("NSTY-00001"), None),
            ("Two Discs.m3u", "dreamcast", Some("NSTY-00003"), None),
        ],
        "no track or member rows"
    );
    let redump = rows
        .iter()
        .find(|g| g.rom_path.ends_with("Redump Game.cue"))
        .unwrap();
    assert_eq!(redump.sha1, track3.sha1, "largest data track = the DAT rom");
    assert_eq!(
        redump.title, "Redump Game",
        "DAT title only on a hash match"
    );
    let sega = rows.iter().find(|g| g.rom_path.ends_with(".chd")).unwrap();
    assert_eq!(
        sega.title, "Sega Title",
        "serial match keeps the file title"
    );
    let size_with_tracks: u64 = ["Redump Game.cue", "Redump Game (Track 1).bin"]
        .iter()
        .chain(["Redump Game (Track 2).bin", "Redump Game (Track 3).bin"].iter())
        .map(|f| std::fs::metadata(roms.join(f)).unwrap().len())
        .sum();
    assert_eq!(
        redump.file_size as u64, size_with_tracks,
        "fingerprint covers tracks"
    );
    // The runner keys saves on the same content id as the scan.
    for game in &rows {
        let spec = if game.platform_id == "psx" {
            &PSX
        } else {
            &DREAMCAST
        };
        let id = game_content_sha1(std::path::Path::new(&game.rom_path), spec).unwrap();
        assert_eq!(id, game.sha1, "{}", game.rom_path);
    }
    assert_eq!(cue.extension().unwrap(), "cue");
}

#[test]
fn two_gdi_games_never_share_a_content_id() {
    let lib = TempLibrary::new("dc-ids");
    let a = write_gdi(&lib.roms(), "a", "NSTY-00001", false);
    let b = write_gdi(&lib.roms(), "b", "NSTY-00009", false);
    let id = |p: &std::path::Path| game_content_sha1(p, &DREAMCAST).unwrap();
    assert_ne!(
        id(&a),
        id(&b),
        "the .gdi text is the same shape; the tracks differ"
    );
}
