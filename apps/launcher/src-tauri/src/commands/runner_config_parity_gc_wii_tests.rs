//! Parity cases for GameCube and Wii on Dolphin (`config/core/dolphin-emu.toml`):
//! proves `core_layer_id("dolphin-emu")` (the dash is kept) is the registry core id
//! and that the runner classifies a `.dol` by its code (`mtspr HID4` → Wii). Tiny
//! in-code DOLs are enough for `--print-config` (the runner first copies Dolphin's
//! `Sys` files into the temp data dir). Skips (exit 6) without OpenGL, like N64.

use disc_image::test_nintendo_images::{dol, MTSPR_HID4_R0};

use super::check_parity;

/// `b .`: the DOL never needs to run (`--print-config` stops after the load).
const BRANCH_TO_SELF: u32 = 0x4800_0000;

fn parity_dol(system: &str, text: &[u32]) {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-dolphin-")
        .tempdir()
        .unwrap();
    let rom = temp.path().join("Parity.dol");
    std::fs::write(&rom, dol(text, &[])).unwrap();
    check_parity(system, &rom);
}

#[test]
fn gc_launcher_resolution_matches_runner_print_config() {
    parity_dol("gc", &[BRANCH_TO_SELF; 8]);
}

#[test]
fn wii_launcher_resolution_matches_runner_print_config() {
    let mut text = [BRANCH_TO_SELF; 8]; // 32 bytes: Dolphin boots aligned sections only
    text[0] = MTSPR_HID4_R0;
    parity_dol("wii", &text);
}
