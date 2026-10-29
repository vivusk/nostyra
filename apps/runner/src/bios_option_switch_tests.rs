//! BIOS switch tests: missing, unknown and known files (a fake core whose known
//! md5 is the md5 of a test pattern, so no real BIOS is needed).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use system_catalog::{BiosOptionSwitch, FLYCAST, SNES9X};

use super::*;

const PATTERN: &[u8] = b"nostyra test bios";

/// Flycast with `dc/dc_boot.bin` known as `PATTERN` (its size and md5).
fn core_knowing_the_pattern() -> CoreSpec {
    let file = FirmwareFile {
        md5: Box::leak(hex(&Md5::digest(PATTERN)).into_boxed_str()),
        size: PATTERN.len() as u64,
        ..FLYCAST.firmware[0]
    };
    CoreSpec {
        firmware: Box::leak(Box::new([file])),
        ..FLYCAST
    }
}

#[test]
fn hle_without_a_file_and_for_unknown_files() {
    let dir = tempfile::tempdir().unwrap();
    let choice = choose(&FLYCAST, dir.path()).unwrap();
    assert_eq!(choice, BiosSwitchChoice::Hle);
    assert_eq!(
        option_for(&FLYCAST, &choice),
        Some(("reicast_hle_bios".into(), "enabled".into()))
    );
    std::fs::create_dir_all(dir.path().join("dc")).unwrap();
    std::fs::write(
        dir.path().join("dc/dc_boot.bin"),
        vec![0u8; 2 * 1024 * 1024],
    )
    .unwrap();
    let choice = choose(&FLYCAST, dir.path()).unwrap();
    assert!(matches!(&choice, BiosSwitchChoice::Unknown(file, _) if file == "dc/dc_boot.bin"));
    assert_eq!(
        option_for(&FLYCAST, &choice).unwrap().1,
        "enabled",
        "never boot a bad BIOS"
    );
    assert!(choice.describe().contains("not a known BIOS"));
}

#[test]
fn a_known_file_selects_the_real_bios() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("dc")).unwrap();
    std::fs::write(dir.path().join("dc/dc_boot.bin"), PATTERN).unwrap();
    let core = core_knowing_the_pattern();
    let choice = choose(&core, dir.path()).unwrap();
    assert_eq!(choice, BiosSwitchChoice::Real("dc/dc_boot.bin".into()));
    assert_eq!(option_for(&core, &choice).unwrap().1, "disabled");
    assert_eq!(choice.describe(), "real BIOS dc/dc_boot.bin");
}

#[test]
fn cores_without_a_switch_are_untouched() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(choose(&SNES9X, dir.path()), None);
    assert_eq!(option_for(&SNES9X, &BiosSwitchChoice::Hle), None);
    let switch: Option<BiosOptionSwitch> = FLYCAST.bios_switch;
    assert!(switch.is_some());
}
