//! Cue-stem hiding rules never hide a cartridge: a Mega Drive `.bin` beside an
//! unrelated same-stem `.cue` stays a game.
#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use std::path::Path;

use disc_image::test_images::{ps1_sectors, to_raw, US_LICENCE};
use game_library::discover_rom_files;
use system_catalog::ALL_SYSTEMS;

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    std::fs::write(root.join(rel), bytes).unwrap();
}

#[test]
fn genesis_bin_beside_a_same_stem_cue_stays_a_game() {
    let dir = tempfile::Builder::new()
        .prefix("nostyra-group-guard-")
        .tempdir()
        .unwrap();
    let mut mega_drive = vec![0u8; 0x20000];
    mega_drive[0x100..0x110].copy_from_slice(b"SEGA MEGA DRIVE ");
    put(dir.path(), "Sonic.bin", &mega_drive);
    let ps1 = to_raw(
        &ps1_sectors(Some(US_LICENCE), r"BOOT = cdrom:\SLUS_012.34;1"),
        2,
    );
    put(dir.path(), "Sonic (Track 1).bin", &ps1);
    put(
        dir.path(),
        "Sonic.cue",
        b"FILE \"Sonic (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
    );
    let found = discover_rom_files(dir.path(), &ALL_SYSTEMS);
    let mut games: Vec<(String, &str)> = found
        .files
        .iter()
        .map(|f| {
            let name = Path::new(&f.path).file_name().unwrap();
            (name.to_string_lossy().into_owned(), f.system.id)
        })
        .collect();
    games.sort();
    assert_eq!(
        games,
        [
            ("Sonic.bin".to_string(), "genesis"),
            ("Sonic.cue".to_string(), "psx")
        ]
    );
    assert!(found.failures.is_empty(), "{:?}", found.failures);
}
