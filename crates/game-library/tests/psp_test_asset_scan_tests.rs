//! The committed PSP test content (`test-assets/roms/src/build-nostyra-test-psp.py`)
//! scans as PSP: `.iso`, `.cso`, `EBOOT.PBP` (+ a DVD `.chd` of the ISO) each give one
//! row with the disc serial; the eboot is titled from its `PARAM.SFO`; the bare
//! `.elf` (tests only) is not a library file.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;

use common::TempLibrary;
use disc_image::test_psp_containers::dvd_chd;
use game_library::DatIndex;
use storage_db::{list_games, GameListQuery};
use system_catalog::PSP;

fn asset(name: &str) -> Vec<u8> {
    let roms = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms");
    std::fs::read(roms.join(name)).unwrap()
}

#[test]
fn committed_psp_test_content_scans_as_psp_with_its_serial() {
    let lib = TempLibrary::new("psp-assets");
    let iso = asset("nostyra-test-psp.iso");
    lib.write("nostyra-test-psp.iso", &iso);
    lib.write("nostyra-test-psp.cso", &asset("nostyra-test-psp.cso"));
    lib.write("nostyra-test-psp.chd", &dvd_chd(&iso, true));
    lib.write("Homebrew/EBOOT.PBP", &asset("nostyra-test-psp/EBOOT.PBP"));
    lib.write("nostyra-test-psp.elf", &asset("nostyra-test-psp.elf"));
    let summary = lib.scan(&DatIndex::default());
    assert!(summary.failures.is_empty(), "{:?}", summary.failures);
    let rows = list_games(&lib.db.lock().unwrap(), &GameListQuery::default()).unwrap();
    let mut got: Vec<(String, &str, Option<&str>, &str)> = rows
        .iter()
        .map(|g| {
            let name = Path::new(&g.rom_path)
                .file_name()
                .unwrap()
                .to_string_lossy();
            (
                name.into_owned(),
                g.platform_id.as_str(),
                g.serial.as_deref(),
                g.title.as_str(),
            )
        })
        .collect();
    got.sort();
    let serial = Some("NSTY-00001");
    assert_eq!(
        got,
        [
            ("EBOOT.PBP".into(), PSP.id, serial, "Nostyra Test PSP"),
            (
                "nostyra-test-psp.chd".into(),
                PSP.id,
                serial,
                "nostyra-test-psp"
            ),
            (
                "nostyra-test-psp.cso".into(),
                PSP.id,
                serial,
                "nostyra-test-psp"
            ),
            (
                "nostyra-test-psp.iso".into(),
                PSP.id,
                serial,
                "nostyra-test-psp"
            ),
        ]
    );
}
