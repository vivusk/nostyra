//! PS2 BIOS option: region from the disc (NTSC serial, PAL, `.m3u` first entry),
//! folder-keyed memo, zero-BIOS refusal before the load (exit 4, no core needed),
//! Flycast's switch untouched.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use disc_image::test_images::to_iso;
use disc_image::test_ps2_images::ps2_volume;
use system_catalog::ps2_bios_test_image::ps2_bios_image;
use system_catalog::{DREAMCAST, PCEE2, PS2};

use super::*;
use crate::bios_option_switch::launch_option;
use crate::runner_exit_codes::{exit_code_for, EXIT_ROM_LOAD};
use crate::runner_session_config::prepare_core;

/// `scan_report`'s memo is one process-wide slot: tests that touch it (here and in
/// `ps2_session_tests`) run one at a time (cargo runs the binary's tests on several
/// threads).
static MEMO_TESTS: Mutex<()> = Mutex::new(());

pub(crate) fn memo_lock() -> MutexGuard<'static, ()> {
    MEMO_TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

const US_160: &str = "scph-39001_bios_v7_usa_160.bin";
const EU_200: &str = "scph-50004_bios_v9_eur_200.bin";

/// A system dir whose `pcsx2/bios` holds a USA v1.60 and a Europe v2.00 dump.
fn system_with_two_dumps() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let bios = temp.path().join("pcsx2/bios");
    std::fs::create_dir_all(&bios).unwrap();
    std::fs::write(bios.join(US_160), ps2_bios_image("0160AC20020207", 0x2740)).unwrap();
    std::fs::write(bios.join(EU_200), ps2_bios_image("0200EC20040614", 0x2740)).unwrap();
    temp
}

/// A PS2 `.iso` whose `SYSTEM.CNF` boots `exe` in `vmode`.
fn disc(dir: &Path, name: &str, exe: &str, vmode: &str) -> PathBuf {
    let cnf = format!("BOOT2 = cdrom0:\\{exe};1\r\nVER = 1.00\r\nVMODE = {vmode}\r\n");
    let iso = to_iso(&ps2_volume(
        "NOSTYRA_PS2",
        &[("SYSTEM.CNF", cnf.as_bytes())],
    ));
    let path = dir.join(name);
    std::fs::write(&path, iso).unwrap();
    path
}

fn value(answer: Option<(String, String)>) -> Option<String> {
    answer.map(|(key, value)| {
        assert_eq!(key, "pcsx2_bios");
        value
    })
}

#[test]
fn region_from_the_disc_picks_the_matching_dump_else_the_newest() {
    let _serial = memo_lock();
    let system = system_with_two_dumps();
    let discs = tempfile::tempdir().unwrap();
    let us = disc(discs.path(), "us.iso", "SLUS_203.12", "NTSC");
    let pal = disc(discs.path(), "pal.iso", "SLES_541.50", "PAL");
    let jp = disc(discs.path(), "jp.iso", "SLPS_254.97", "NTSC");
    let homebrew = disc(discs.path(), "hb.iso", "BOOT.ELF", "NTSC");
    let launch = |rom: &Path| value(launch_option(&PS2, system.path(), rom));
    assert_eq!(launch(&us).as_deref(), Some(US_160));
    assert_eq!(launch(&pal).as_deref(), Some(EU_200));
    assert_eq!(launch(&jp).as_deref(), Some(EU_200), "no JP dump: newest");
    assert_eq!(
        launch(&homebrew).as_deref(),
        Some(EU_200),
        "no region: newest"
    );
    // `.m3u`: the first entry's region decides.
    let m3u = discs.path().join("set.m3u");
    std::fs::write(&m3u, "us.iso\r\npal.iso\r\n").unwrap();
    assert_eq!(launch(&m3u).as_deref(), Some(US_160));
    assert_eq!(launch(Path::new("missing.iso")).as_deref(), Some(EU_200));
    assert_eq!(
        option(&PS2, system.path(), &system_catalog::SNES9X, &us),
        None
    );
}

#[test]
fn report_is_memoised_per_folder_not_per_process() {
    let _serial = memo_lock();
    let a = system_with_two_dumps();
    let b = tempfile::tempdir().unwrap();
    let first = scan_report(a.path(), &PCEE2);
    assert_eq!(first.files.len(), 2);
    // A file added after the scan is not seen again for the same folder: one scan
    // per launch (the launcher's own view rescans in its process).
    std::fs::write(
        a.path().join("pcsx2/bios/late.bin"),
        ps2_bios_image("0230AC20080220", 0x2780),
    )
    .unwrap();
    assert_eq!(scan_report(a.path(), &PCEE2), first, "memo hit");
    // Another folder gets its own scan (never the first test's report)...
    assert!(
        scan_report(b.path(), &PCEE2).files.is_empty(),
        "other folder"
    );
    // ...and replaces the slot, so the first folder is scanned afresh afterwards.
    assert_eq!(scan_report(a.path(), &PCEE2).files.len(), 3, "rescanned");
}

#[test]
fn zero_bios_is_refused_before_the_core_loads_with_exit_4() {
    let _serial = memo_lock();
    let empty = tempfile::tempdir().unwrap();
    let core = empty.path().join("cores/pcee2_libretro.dll"); // never opened
    let rom = empty.path().join("game.iso");
    let err = prepare_core(&PS2, &core, empty.path(), &rom).unwrap_err();
    assert_eq!(exit_code_for(&err), EXIT_ROM_LOAD);
    let text = err.to_string();
    assert!(text.contains("needs your BIOS dump"), "{text}");
    assert!(text.contains("Settings"), "{text}");
    assert!(text.contains("game.iso"), "{text}");
    assert_eq!(
        launch_option(&PS2, empty.path(), &rom),
        None,
        "nothing to answer"
    );
    let with_dump = system_with_two_dumps();
    prepare_core(&PS2, &core, with_dump.path(), &rom).expect("a dump present");
}

#[test]
fn bios_switch_cores_still_take_the_switch_path() {
    let dir = tempfile::tempdir().unwrap();
    let flycast = launch_option(&DREAMCAST, dir.path(), Path::new("game.gdi"));
    assert_eq!(flycast, Some(("reicast_hle_bios".into(), "enabled".into())));
    assert_eq!(
        launch_option(&system_catalog::PSX, dir.path(), Path::new("game.cue")),
        None,
        "PCSX ReARMed checks its BIOS itself"
    );
}
