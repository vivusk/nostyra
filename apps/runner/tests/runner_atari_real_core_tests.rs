//! Atari end to end through the runner binary (M17): the Lynx cartridge EEPROM is
//! Handy's own `content.eeprom` in the per-game save folder, loaded and written back
//! by the next process; a BS93 `.o` writes no save file; a soft patch reaches Handy
//! through the staged content file; a headerless 7800 `.bin` boots with `--system`
//! and, without it, gets the PlayStation reason (the launcher always passes it).
//!
//! Skips with a note when a core is missing (`scripts/fetch-{handy,prosystem}-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};

use support::{repo, result_line, runner, scratch};

const FRAMES: [&str; 2] = ["--headless-frames", "30"];

/// The test ROM `file` copied into a fresh scratch folder (patches go beside it).
fn rom_copy(tag: &str, file: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(tag);
    let rom = dir.join(file);
    std::fs::copy(repo(&format!("test-assets/roms/{file}")), &rom).unwrap();
    (rom, dir.join("data"))
}

fn run_ok(core: &Path, rom: &Path, data: &Path, extra: &[&str]) -> String {
    let out = runner(core, rom, data, &[&FRAMES[..], extra].concat());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{}: {stderr}", rom.display());
    result_line(&out)
}

#[test]
fn lynx_eeprom_is_content_eeprom_and_survives_the_next_process() {
    let Some(handy) = support::registry_core("handy", "fetch-handy-core") else {
        return;
    };
    let (rom, data) = rom_copy("atari-lynx-eeprom", "nostyra-test.lnx");
    run_ok(&handy, &rom, &data, &["--system", "lynx"]);
    let eeprom = support::game_saves_dir(&data, "lynx", &rom).join("content.eeprom");
    assert_eq!(std::fs::metadata(&eeprom).unwrap().len(), 128, "93C46");
    // The next process loads the file and writes the same bytes back at deinit.
    let marker: Vec<u8> = (0..128u8).collect();
    std::fs::write(&eeprom, &marker).unwrap();
    run_ok(&handy, &rom, &data, &["--system", "lynx"]);
    assert_eq!(std::fs::read(&eeprom).unwrap(), marker);
}

#[test]
fn lynx_bs93_writes_no_save_and_a_soft_patch_reaches_handy() {
    let Some(handy) = support::registry_core("handy", "fetch-handy-core") else {
        return;
    };
    let (rom, data) = rom_copy("atari-lynx-o", "nostyra-test.o");
    let clean = run_ok(&handy, &rom, &data, &["--system", "lynx"]);
    let saves = support::game_saves_dir(&data, "lynx", &rom);
    let files: Vec<_> = std::fs::read_dir(&saves).map_or(Vec::new(), |dir| {
        dir.flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".srm") || name.ends_with(".eeprom"))
            .collect()
    });
    assert!(files.is_empty(), "no SRAM / EEPROM file: {files:?}");
    // IPS turning the red `LDA #$0F` into `LDA #$05`: another picture.
    let bytes = std::fs::read(&rom).unwrap();
    let at = bytes.windows(2).position(|w| w == [0xA9, 0x0F]).unwrap() + 1;
    let mut ips = b"PATCH".to_vec();
    ips.extend_from_slice(&u32::try_from(at).unwrap().to_be_bytes()[1..]);
    ips.extend_from_slice(&[0x00, 0x01, 0x05]);
    ips.extend_from_slice(b"EOF");
    std::fs::write(rom.with_extension("ips"), ips).unwrap();
    let patched = run_ok(&handy, &rom, &data, &["--system", "lynx"]);
    assert_ne!(patched, clean, "patched bytes reach Handy (staged content)");
}

#[test]
fn headerless_7800_bin_boots_with_its_system() {
    let Some(prosystem) = support::registry_core("prosystem", "fetch-prosystem-core") else {
        return;
    };
    let (rom, data) = rom_copy("atari-7800-bin", "nostyra-test-7800.bin");
    run_ok(&prosystem, &rom, &data, &["--system", "atari7800"]);
    // Without --system the runner classifies by content: no DAT, so the last disc
    // candidate's (PlayStation 2) text.
    let out = runner(&prosystem, &rom, &data, &FRAMES);
    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not a PlayStation or PlayStation 2 disc"),
        "{stderr}"
    );
}
