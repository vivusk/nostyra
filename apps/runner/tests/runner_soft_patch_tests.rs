//! Soft-patching end to end: a patch named like the ROM changes what the game runs;
//! a patch for another ROM is skipped and the game starts unpatched. Either way state
//! sidecars record the patch file's CRC (resume-point "patch changed" warning).
//!
//! Requires the snes9x core fetched by `scripts/fetch-snes9x-core.{sh,ps1}`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};

use support::{core, result_line, runner, scratch, test_rom};

/// Copies the test ROM into `dir` as `game.sfc` and returns its path.
fn rom_copy(dir: &Path) -> PathBuf {
    let rom = dir.join("game.sfc");
    std::fs::copy(test_rom(), &rom).unwrap();
    rom
}

/// IPS that zeroes the first 0x7000 bytes (the program) → different output.
fn blanking_ips() -> Vec<u8> {
    let mut patch = b"PATCH".to_vec();
    for offset in (0..0x7000u32).step_by(0x1000) {
        patch.extend_from_slice(&offset.to_be_bytes()[1..]);
        patch.extend_from_slice(&[0x10, 0x00]);
        patch.extend(std::iter::repeat_n(0u8, 0x1000));
    }
    patch.extend_from_slice(b"EOF");
    patch
}

fn headless(rom: &Path, data: &Path) -> String {
    headless_with(rom, data, &[])
}

fn headless_with(rom: &Path, data: &Path, extra: &[&str]) -> String {
    let args = [&["--headless-frames", "120"], extra].concat();
    let out = runner(&core(), rom, data, &args);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    result_line(&out)
}

#[test]
fn matching_ips_is_applied_and_mismatched_bps_is_skipped() {
    let clean_dir = scratch("patch-clean");
    let clean = headless(&rom_copy(&clean_dir), &clean_dir.join("data"));

    let patched_dir = scratch("patch-ips");
    let rom = rom_copy(&patched_dir);
    std::fs::write(patched_dir.join("game.ips"), blanking_ips()).unwrap();
    let patched = headless(&rom, &patched_dir.join("data"));
    assert_ne!(patched, clean, "IPS must change the running game");
    assert!(
        std::fs::read(&rom).unwrap() == std::fs::read(test_rom()).unwrap(),
        "ROM file untouched"
    );

    // BPS made for another ROM (source CRC 0): checksum mismatch → unpatched game.
    let wrong_dir = scratch("patch-wrong-bps");
    let rom = rom_copy(&wrong_dir);
    let mut bps = b"BPS1\x80\x80\x80".to_vec();
    bps.extend_from_slice(&[0; 8]);
    let own = crc32fast::hash(&bps);
    bps.extend_from_slice(&own.to_le_bytes());
    std::fs::write(wrong_dir.join("game.bps"), &bps).unwrap();
    assert_eq!(headless(&rom, &wrong_dir.join("data")), clean);

    // Failed patch: the sidecar still records the CRC of the patch file found.
    let data = wrong_dir.join("data");
    headless_with(&rom, &data, &["--headless-save-slot", "auto"]);
    let sidecar = support::game_saves_dir(&data, "snes", &rom).join("auto.json");
    let sidecar: ipc_protocol::StateSidecar =
        serde_json::from_slice(&std::fs::read(sidecar).unwrap()).unwrap();
    assert_eq!(sidecar.rom_patch_crc32, Some(crc32fast::hash(&bps)));
}
