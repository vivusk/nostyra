//! Parity case for PlayStation 2 on PCEE2 (`config/core/pcee2.toml`): proves
//! `core_layer_id("PCEE2") == "pcee2"`, the registry core id, and that the runner
//! classifies a `.iso` as `ps2` by its `SYSTEM.CNF`. PCEE2 has no HLE BIOS and the
//! runner refuses to load without one (exit 4), so the case needs a real dump: it
//! runs only with `NOSTYRA_PS2_BIOS_DIR` pointing at a folder holding the user's
//! BIOS file(s) (copied into the temp data dir's `system/pcsx2/bios`, never into the
//! repo) and skips with a note otherwise. The disc is the in-code test volume
//! (`--print-config` stops after the load). Skips (exit 6) without OpenGL, like N64.

use std::path::Path;

use disc_image::test_images::to_iso;
use disc_image::test_ps2_images::{ps2_sectors, TEST_EXE};

use super::check_parity;

/// Folder with the user's PS2 BIOS dump(s); same gate as the runner's PS2 suites.
const BIOS_DIR_ENV: &str = "NOSTYRA_PS2_BIOS_DIR";

/// Copies every regular file of `bios_dir` into `<data>/system/pcsx2/bios/` (the
/// scan keeps the 4–8 MiB ROMDIR dumps and ignores `.nvm` / `.mec` / `.rom1`).
fn install_bios(bios_dir: &Path, data: &Path) {
    let target = data.join("system").join("pcsx2").join("bios");
    std::fs::create_dir_all(&target).unwrap();
    for entry in std::fs::read_dir(bios_dir).unwrap().flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_file()) {
            std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}

#[test]
fn ps2_launcher_resolution_matches_runner_print_config() {
    let Some(bios_dir) = std::env::var_os(BIOS_DIR_ENV).filter(|dir| !dir.is_empty()) else {
        eprintln!("skipped: set {BIOS_DIR_ENV} to a folder with your PS2 BIOS dump");
        return;
    };
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-ps2-disc-")
        .tempdir()
        .unwrap();
    let iso = temp.path().join("parity.iso");
    std::fs::write(&iso, to_iso(&ps2_sectors(TEST_EXE))).unwrap();
    check_parity("ps2", &iso, |data| install_bios(Path::new(&bios_dir), data));
}
