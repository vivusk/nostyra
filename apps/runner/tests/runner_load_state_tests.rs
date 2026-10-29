//! Save-state thumbnails and `--load-state`, end to end through the headless runner:
//! `--headless-save-slot` writes `slotN.state/.json/.png` (or `auto.*`, the resume
//! point), `--load-state` restores it before frame 1, and a missing or broken slot
//! never stops the game (exit 0).
//!
//! Requires the snes9x core fetched by `scripts/fetch-snes9x-core.{sh,ps1}`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::Path;
use std::process::Output;

use support::{core, result_line, runner, scratch, test_rom};

fn run_ok(data: &Path, extra: &[&str]) -> (String, String) {
    let out: Output = runner(&core(), &test_rom(), data, extra);
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    (result_line(&out), stderr)
}

/// `checksum=<hex>` field of a result line.
fn checksum(line: &str) -> &str {
    line.split(' ')
        .find(|field| field.starts_with("checksum="))
        .expect("checksum field")
}

#[test]
fn saved_slot_has_thumbnail_and_loads_before_first_frame() {
    let data = scratch("load-state-roundtrip");
    let (saved, _) = run_ok(
        &data,
        &["--headless-frames", "120", "--headless-save-slot", "0"],
    );
    assert!(saved.ends_with("loaded=false"), "{saved}");
    let slots = support::game_saves_dir(&data, "snes", &test_rom());
    for file in ["slot0.state", "slot0.json"] {
        assert!(slots.join(file).metadata().unwrap().len() > 0, "{file}");
    }
    let png = std::fs::read(slots.join("slot0.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "thumbnail is a PNG");
    // Native core frame size (Snes9x: 256×224 on the test ROM), decodable, not blank.
    let thumb = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(thumb.dimensions(), (256, 224), "native-size thumbnail");
    assert!(
        thumb.pixels().any(|px| px.0[..3] != [0, 0, 0]),
        "thumbnail shows the frame"
    );
    let leftovers = std::fs::read_dir(&slots)
        .unwrap()
        .filter(|entry| entry.as_ref().unwrap().path().extension() == Some("tmp".as_ref()))
        .count();
    assert_eq!(leftovers, 0, "no temp files left");

    let (cold, _) = run_ok(&data, &["--headless-frames", "1"]);
    let (warm, _) = run_ok(&data, &["--headless-frames", "1", "--load-state", "0"]);
    assert!(warm.ends_with("loaded=true"), "{warm}");
    assert!(cold.ends_with("loaded=false"), "{cold}");
    assert_ne!(checksum(&warm), checksum(&cold), "load must change frame 1");
}

#[test]
fn missing_or_broken_slot_starts_the_game_normally() {
    let data = scratch("load-state-missing");
    let (line, stderr) = run_ok(&data, &["--headless-frames", "1", "--load-state", "5"]);
    assert!(line.ends_with("loaded=false"), "{line}");
    assert!(
        stderr.contains("ERROR") && stderr.contains("empty"),
        "{stderr}"
    );

    let slots = support::game_saves_dir(&data, "snes", &test_rom());
    std::fs::write(slots.join("slot3.state"), [1u8, 2, 3]).unwrap();
    std::fs::write(slots.join("slot3.json"), b"{ not json").unwrap();
    let (line, stderr) = run_ok(&data, &["--headless-frames", "1", "--load-state", "3"]);
    assert!(line.ends_with("loaded=false"), "{line}");
    assert!(stderr.contains("corrupt state metadata"), "{stderr}");
}

#[test]
fn load_state_conflicts_with_state_slot() {
    let data = scratch("load-state-conflict");
    let extra = [
        "--load-state",
        "1",
        "--state-slot",
        "2",
        "--headless-frames",
        "1",
    ];
    let out = runner(&core(), &test_rom(), &data, &extra);
    assert_eq!(out.status.code(), Some(2), "clap usage error");
}

/// Resume point round trip: a state saved as `auto` after 900 frames, loaded by a new
/// process, gives the same frame as an uninterrupted 901-frame run (exact restore).
/// The 600-frame exit rule is a policy unit test (headless never runs `finish()`).
#[test]
fn auto_slot_resumes_exactly() {
    let data = scratch("load-state-auto");
    let (saved, _) = run_ok(
        &data,
        &["--headless-frames", "900", "--headless-save-slot", "auto"],
    );
    assert!(saved.ends_with("loaded=false"), "{saved}");
    let dir = support::game_saves_dir(&data, "snes", &test_rom());
    for file in ["auto.state", "auto.json", "auto.png"] {
        assert!(dir.join(file).metadata().unwrap().len() > 0, "{file}");
    }
    assert!(
        !dir.join("slot0.state").exists(),
        "auto never touches slot 0"
    );
    let sidecar: ipc_protocol::StateSidecar =
        serde_json::from_slice(&std::fs::read(dir.join("auto.json")).unwrap()).unwrap();
    let srm = std::fs::read(dir.join(ipc_protocol::SRAM_FILE_NAME)).unwrap();
    assert_eq!(
        sidecar.sram_crc32,
        Some(crc32fast::hash(&srm)),
        "CRC of the flushed SRAM"
    );
    assert_eq!(sidecar.rom_patch_crc32, None, "no patch file");

    let (resumed, _) = run_ok(&data, &["--headless-frames", "1", "--load-state", "auto"]);
    assert!(resumed.ends_with("loaded=true"), "{resumed}");
    let reference = run_ok(
        &scratch("load-state-auto-ref"),
        &["--headless-frames", "901"],
    )
    .0;
    assert_eq!(checksum(&resumed), checksum(&reference), "identical state");
}

#[test]
fn missing_auto_slot_starts_fresh() {
    let data = scratch("load-state-auto-missing");
    let (line, stderr) = run_ok(&data, &["--headless-frames", "1", "--load-state", "auto"]);
    assert!(line.ends_with("loaded=false"), "{line}");
    assert!(stderr.contains("slot=auto"), "{stderr}");
}
