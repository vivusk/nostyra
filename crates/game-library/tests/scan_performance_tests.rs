//! Exit criterion: 500 SNES ROMs → first scan < 10 s, unchanged rescan < 1 s.
//! Files are in the OS page cache right after being written, so this is not a cold-disk
//! number; the real cold-SSD scan is a manual check (phase 07).
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::time::Instant;

use common::{fake_rom, TempLibrary};
use game_library::DatIndex;

#[test]
fn five_hundred_one_megabyte_roms() {
    let lib = TempLibrary::new("perf");
    for i in 0..500 {
        lib.write(&format!("game {i:03}.sfc"), &fake_rom(1024 * 1024, i));
    }
    let dat = DatIndex::empty();

    let start = Instant::now();
    let first = lib.scan(&dat);
    let first_elapsed = start.elapsed();
    eprintln!("first scan: {} files in {first_elapsed:?}", first.added);
    assert_eq!(first.added, 500);
    assert!(
        first_elapsed.as_secs_f64() < 10.0,
        "first scan took {first_elapsed:?}"
    );

    let start = Instant::now();
    let rescan = lib.scan(&dat);
    let rescan_elapsed = start.elapsed();
    eprintln!(
        "unchanged rescan: {} files in {rescan_elapsed:?}",
        rescan.unchanged
    );
    assert_eq!(rescan.unchanged, 500);
    assert!(
        rescan_elapsed.as_secs_f64() < 1.0,
        "rescan took {rescan_elapsed:?}"
    );
}
