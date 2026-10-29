//! Vendored Redump GameCube / Wii data found through the game-ID serial rule: a disc
//! header's `GALE01` / `RSPE01` matches Redump `DL-DOL-GALE-USA` / `RVL-RSPE-USA` by
//! the 4-char game code; the metadat attributes are header-only placeholders
//! (libretro-database has none for these systems). Files no GC / Wii probe claims
//! are one scan-problem row each, with the last candidate's reject text.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;

use common::TempLibrary;
use game_library::{DatIndex, SystemCatalogs};
use system_catalog::{GC, WII};

#[test]
fn vendored_gamecube_and_wii_dats_match_game_ids() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    let gc = catalogs.get(GC.id);
    let wii = catalogs.get(WII.id);
    assert!(gc.dat.len() > 2_000, "{}", gc.dat.len());
    assert!(wii.dat.len() > 3_000, "{}", wii.dat.len());
    assert!(
        gc.metadata.is_complete() && wii.metadata.is_complete(),
        "placeholders"
    );
    // Regions share a game code (Korea / USA Melee are both GALE): first in DAT order.
    let melee = gc.dat.lookup_serial("GALE01", None).expect("GALE");
    assert!(
        melee.name.starts_with("Super Smash Bros. Melee ("),
        "{}",
        melee.name
    );
    assert!(melee.serial.as_deref().unwrap().starts_with("DL-DOL-GALE-"));
    let sports = wii.dat.lookup_serial("RSPE01", None).expect("RSPE");
    assert!(
        sports.name.starts_with("Wii Sports (USA)"),
        "{}",
        sports.name
    );
    // Another region's code does not match.
    assert!(gc.dat.lookup_serial("ZZZZ01", None).is_none());
    assert_ne!(
        gc.dat.lookup_serial("GALP01", None).map(|e| &e.name),
        Some(&melee.name)
    );
}

#[test]
fn foreign_iso_and_headerless_containers_are_one_problem_row_each() {
    let lib = TempLibrary::new("gcwii-bad");
    lib.write("Pc Dvd.iso", &vec![0u8; 0x9000]);
    lib.write("Not Wii.wbfs", &vec![0u8; 0x400]);
    lib.write("Not Gc.rvz", &vec![0u8; 0x400]);
    let summary = lib.scan(&DatIndex::default());
    assert_eq!(summary.added, 0);
    let mut reasons: Vec<(String, String)> = summary
        .failures
        .iter()
        .map(|f| {
            let name = f.path.file_name().unwrap().to_str().unwrap().to_owned();
            (name, f.reason.clone())
        })
        .collect();
    reasons.sort();
    assert_eq!(
        reasons,
        [
            ("Not Gc.rvz".into(), "not a GameCube or Wii disc".into()),
            ("Not Wii.wbfs".into(), "not a Wii disc".into()),
            (
                "Pc Dvd.iso".into(),
                "not a PlayStation, PSP, GameCube or Wii disc".into()
            ),
        ]
    );
}
