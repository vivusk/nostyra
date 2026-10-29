//! PSP identification end to end, in a tree mixed with PlayStation and Dreamcast
//! discs: every container classified by content, identity = whole file (Redump SHA1
//! match for `.iso`), serial from `UMD_DATA.BIN` / `PARAM.SFO` matched through the
//! PSP rule (`NSTY-00001/E`, `ULUS10041`), eboots titled from `PARAM.SFO`, scan ==
//! runner content id. Unreadable images and vendored data: `psp_vendored_data_tests`.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;
use std::sync::atomic::AtomicBool;

use common::TempLibrary;
use disc_image::test_gd_images::gd_chd;
use disc_image::test_images::{pbp_image, ps1_sectors, to_iso, US_LICENCE};
use disc_image::test_psp_containers::{cso, dvd_chd, pbp};
use disc_image::test_psp_images::{game_sfo, iso_tree, psp_iso_sectors, sfo, SfoValue};
use game_library::{
    game_content_sha1, hash_file_streaming, scan_library, DatEntry, DatIndex, MetadataIndex,
    ScanSummary, SystemCatalog, SystemCatalogs,
};
use storage_db::{list_games, Game, GameListQuery};
use system_catalog::{system, DiscSignature, PSP};

fn entry(name: &str, serial: &str, sha1: &str) -> DatEntry {
    DatEntry {
        name: name.into(),
        region: None,
        size: 1,
        crc32: "00000000".into(),
        sha1: Some(sha1.into()),
        serial: Some(serial.into()),
        metadata_crc32: "00000000".into(),
    }
}

fn scan(lib: &TempLibrary, psp_dat: DatIndex) -> (ScanSummary, Vec<Game>) {
    let rule = Some(DiscSignature::Psp);
    let catalogs = SystemCatalogs::from_loaded([(
        PSP.id,
        SystemCatalog {
            dat: psp_dat.with_serial_rule(rule),
            metadata: MetadataIndex::empty().with_serial_rule(rule),
        },
    )]);
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

/// (parent/file, platform, serial, DAT name, title) of a scanned row.
type Row<'a> = (String, &'a str, Option<&'a str>, Option<&'a str>, &'a str);

fn file_name(path: &str) -> String {
    let path = Path::new(path);
    let parent = path
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    format!("{parent}/{}", path.file_name().unwrap().to_str().unwrap())
}

#[test]
fn mixed_tree_classifies_every_psp_container_and_matches_the_dat() {
    let lib = TempLibrary::new("psp-id");
    let iso = to_iso(&psp_iso_sectors());
    let iso_path = lib.write("Test.iso", &iso);
    lib.write("Test.cso", &cso(&iso, 2048, 0));
    lib.write("Test.chd", &dvd_chd(&iso, true));
    lib.write("Homebrew/EBOOT.PBP", &pbp(&game_sfo("MG"), b""));
    let untitled = sfo(&[
        ("CATEGORY", SfoValue::Str("MG")),
        ("DISC_ID", SfoValue::Str("NSTY00002")),
    ]);
    lib.write("Untitled Demo/EBOOT.PBP", &pbp(&untitled, b""));
    let us_sfo = sfo(&[
        ("CATEGORY", SfoValue::Str("UG")),
        ("DISC_ID", SfoValue::Str("ULUS10041")),
    ]);
    lib.write(
        "Us Game.iso",
        &to_iso(&iso_tree("PSP GAME", &[("PSP_GAME/PARAM.SFO", &us_sfo)])),
    );
    lib.write("Ps1/EBOOT.PBP", &pbp_image("ME", "SLUS00594"));
    lib.write(
        "Ps1 Game.iso",
        &to_iso(&ps1_sectors(
            Some(US_LICENCE),
            r"BOOT = cdrom:\SLUS_012.34;1",
        )),
    );
    lib.write("Dc Game.chd", &gd_chd("MK-51035"));
    let sha1 = hash_file_streaming(&iso_path).unwrap().sha1;
    let dat = DatIndex::from_entries(vec![
        entry("Nostyra Test PSP (World)", "NSTY-00001/E", &sha1),
        entry("Nostyra Test PSP (World) (Alt)", "NSTY-00001", &sha1),
        entry("Us Game (USA)", "ULUS-10041", "AA"),
    ]);
    let (summary, rows) = scan(&lib, dat);
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    let mut got: Vec<Row> = rows
        .iter()
        .map(|g| {
            let name = file_name(&g.rom_path);
            (
                name,
                g.platform_id.as_str(),
                g.serial.as_deref(),
                g.dat_name.as_deref(),
                g.title.as_str(),
            )
        })
        .collect();
    got.sort();
    let world = Some("Nostyra Test PSP (World)");
    let root = lib.roms().file_name().unwrap().to_str().unwrap().to_owned();
    let at = |name: &str| format!("{root}/{name}");
    assert_eq!(
        got,
        [
            (
                "Homebrew/EBOOT.PBP".into(),
                "psp",
                Some("NSTY-00001"),
                world,
                "Nostyra Test PSP"
            ),
            (
                "Ps1/EBOOT.PBP".into(),
                "psx",
                Some("SLUS-00594"),
                None,
                "EBOOT"
            ),
            (
                "Untitled Demo/EBOOT.PBP".into(),
                "psp",
                Some("NSTY-00002"),
                None,
                "Untitled Demo"
            ),
            (
                at("Dc Game.chd"),
                "dreamcast",
                Some("MK-51035"),
                None,
                "Dc Game"
            ),
            (
                at("Ps1 Game.iso"),
                "psx",
                Some("SLUS-01234"),
                None,
                "Ps1 Game"
            ),
            (at("Test.chd"), "psp", Some("NSTY-00001"), world, "Test"),
            (at("Test.cso"), "psp", Some("NSTY-00001"), world, "Test"),
            (
                at("Test.iso"),
                "psp",
                Some("NSTY-00001"),
                world,
                "Nostyra Test PSP"
            ),
            (
                at("Us Game.iso"),
                "psp",
                Some("ULUS-10041"),
                Some("Us Game (USA)"),
                "Us Game"
            ),
        ]
    );
    for game in &rows {
        let spec = system(&game.platform_id).unwrap();
        let id = game_content_sha1(Path::new(&game.rom_path), spec).unwrap();
        assert_eq!(id, game.sha1, "{}", game.rom_path);
    }
}
