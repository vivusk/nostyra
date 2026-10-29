//! Nintendo 64 end to end through the runner binary on Mupen64Plus-Next with the
//! self-written `nostyra-test.z64`, on the Angrylion software renderer (hidden
//! `--core-option`), so it runs without OpenGL: every run sets `NOSTYRA_FORCE_NO_GL=1`
//! (debug runner), which would fail any GL context. The EEPROM boot counter persists
//! in the 296 960-byte `save.srm` (no `.srm.bak`); `.v64` / `.n64` copies share the
//! `.z64` save folder; a slot saved headless loads with `--load-state` although Mupen
//! refuses states before its first frame (startup retry for `Serialized` cores), and
//! a truncated slot is refused by size instead of hanging the runner.
//! Exit 6 and the GL smoke run are in `runner_n64_gl_tests`.
//!
//! Skips with a note when the core is missing (`scripts/fetch-mupen64plus-next-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use system_catalog::{n64_to_order, N64ByteOrder, NINTENDO_64};

use support::{repo, result_line, scratch};

const SAVE_RAM_BYTES: usize = 296_960;

fn mupen() -> Option<PathBuf> {
    let core = system_catalog::core_for_system(NINTENDO_64.id).unwrap();
    support::registry_core(core.id, "fetch-mupen64plus-next-core")
}

fn test_rom() -> PathBuf {
    repo("test-assets/roms/nostyra-test.z64")
}

/// Runs the runner headless on Angrylion with GL forced off; exit 0 expected.
fn run_ok(core: &Path, rom: &Path, data: &Path, extra: &[&str]) -> (String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .env("NOSTYRA_FORCE_NO_GL", "1")
        .arg("--core")
        .arg(core)
        .arg("--rom")
        .arg(rom)
        .arg("--data-dir")
        .arg(data)
        .args(["--system", NINTENDO_64.id])
        .args(["--core-option", "mupen64plus-rdp-plugin=angrylion"])
        .args(extra)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(0), "{}: {stderr}", rom.display());
    (result_line(&out), stderr)
}

fn saves_dir(data: &Path, rom: &Path) -> PathBuf {
    support::game_saves_dir(data, NINTENDO_64.id, rom)
}

/// `save.srm` of `rom`: full SAVE_RAM size, EEPROM marker in place; returns the
/// boot counter.
fn boot_counter(data: &Path, rom: &Path) -> u8 {
    let dir = saves_dir(data, rom);
    let srm = std::fs::read(dir.join(ipc_protocol::SRAM_FILE_NAME)).unwrap();
    assert_eq!(srm.len(), SAVE_RAM_BYTES, "whole SAVE_RAM blob");
    assert_eq!(srm[1], 0xA5, "EEPROM marker");
    assert!(!dir.join("save.srm.bak").exists(), "no .srm.bak");
    srm[0]
}

#[test]
fn eeprom_counter_persists_without_opengl() {
    let Some(core) = mupen() else { return };
    let data = scratch("n64-eeprom");
    let rom = test_rom();
    let frames = ["--headless-frames", "30"];
    let (line, _) = run_ok(&core, &rom, &data, &frames);
    assert!(line.contains(" nonblack=true "), "{line}");
    assert_eq!(boot_counter(&data, &rom), 1, "first boot on a blank EEPROM");
    run_ok(&core, &rom, &data, &frames);
    assert_eq!(boot_counter(&data, &rom), 2, "counter survived the restart");
}

#[test]
fn v64_and_n64_copies_share_the_z64_save_folder() {
    let Some(core) = mupen() else { return };
    let data = scratch("n64-byte-orders");
    let roms = data.join("roms");
    std::fs::create_dir_all(&roms).unwrap();
    let z64_bytes = std::fs::read(test_rom()).unwrap();
    let z64 = roms.join("Game.z64");
    std::fs::write(&z64, &z64_bytes).unwrap();
    let frames = ["--headless-frames", "30"];
    run_ok(&core, &z64, &data, &frames);
    let copies = [
        ("Game.v64", N64ByteOrder::ByteSwapped),
        ("Game.n64", N64ByteOrder::LittleEndian),
    ];
    for (expected, (name, order)) in (2u8..).zip(copies) {
        let copy = roms.join(name);
        std::fs::write(&copy, n64_to_order(&z64_bytes, order)).unwrap();
        assert_eq!(saves_dir(&data, &copy), saves_dir(&data, &z64), "{name}");
        run_ok(&core, &copy, &data, &frames);
        assert_eq!(boot_counter(&data, &z64), expected, "{name} continued");
    }
}

/// Mupen refuses `unserialize` before its first frame; the runner retries the
/// startup load after each frame (`Serialized` states), so the slot still loads.
#[test]
fn saved_slot_loads_after_the_first_frame_retry() {
    let Some(core) = mupen() else { return };
    let data = scratch("n64-load-state");
    let rom = test_rom();
    let checksum = |line: &str| {
        line.split(' ')
            .find(|f| f.starts_with("checksum="))
            .unwrap()
            .to_string()
    };
    let save = ["--headless-frames", "90", "--headless-save-slot", "1"];
    let (saved, _) = run_ok(&core, &rom, &data, &save);
    assert!(saved.ends_with("loaded=false"), "{saved}");
    assert!(saves_dir(&data, &rom).join("slot1.state").is_file());
    let load = ["--headless-frames", "5", "--load-state", "1"];
    let (warm, stderr) = run_ok(&core, &rom, &data, &load);
    assert!(warm.ends_with("loaded=true"), "{warm}\n{stderr}");
    let (cold, _) = run_ok(&core, &rom, &data, &["--headless-frames", "5"]);
    assert!(cold.ends_with("loaded=false"), "{cold}");
    assert_ne!(checksum(&warm), checksum(&cold), "slot loaded");
    // A truncated slot is refused by size (Mupen would never return on it).
    let state = saves_dir(&data, &rom).join("slot1.state");
    let bytes = std::fs::read(&state).unwrap();
    std::fs::write(&state, &bytes[..bytes.len() / 2]).unwrap();
    let (broken, stderr) = run_ok(&core, &rom, &data, &load);
    assert!(broken.ends_with("loaded=false"), "{broken}\n{stderr}");
}
