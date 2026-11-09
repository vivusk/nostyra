//! Gate of the PlayStation 2 real-core tests: PCEE2 has no HLE BIOS, so they run
//! only where `NOSTYRA_PS2_BIOS_DIR` names a flat folder holding the user's dump(s)
//! (one `SCPH-xxxxx` folder, or the imported `<data>/system/pcsx2/bios`). The files
//! that parse as a BIOS (phase-02 ROMDIR rule) are copied into the test's own
//! `<system>/pcsx2/bios/<stem lower>.bin`; `.NVM` / `.MEC` / `.ROM1` / `.DIFF`
//! sidecars never are, so a `.nvm` / `.mec` found beside the copy afterwards was
//! written by the core. The copy lives in a temp folder and is deleted with it; the
//! BIOS is never read from the repo. Included once by `#[path]` from
//! `achievements_real_core_loader` (no `main.rs` module line); the PS2 suite uses it
//! from there.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use system_catalog::{firmware_dir, parse_ps2_bios, PCEE2, PS2_BIOS_MAX_BYTES, PS2_BIOS_MIN_BYTES};

/// Folder with the user's PS2 BIOS dump(s).
const BIOS_DIR_ENV: &str = "NOSTYRA_PS2_BIOS_DIR";
/// The skip reason (printed after "skipped: " by the suites).
pub(crate) const NEEDS_BIOS: &str = "set NOSTYRA_PS2_BIOS_DIR to a folder with your PS2 BIOS";

/// Copies every valid BIOS dump of `$NOSTYRA_PS2_BIOS_DIR` into `system_dir`'s PCEE2
/// BIOS folder as `<stem lower>.bin`; the stored stems, or the skip reason when the
/// variable is unset / empty or no file in the folder parses as a BIOS.
pub(crate) fn install_user_bios(system_dir: &Path) -> Result<Vec<String>, &'static str> {
    let dir = std::env::var_os(BIOS_DIR_ENV)
        .filter(|dir| !dir.is_empty())
        .ok_or(NEEDS_BIOS)?;
    let target = firmware_dir(system_dir, &PCEE2);
    std::fs::create_dir_all(&target).unwrap();
    let mut stems = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if !path.is_file() || !(PS2_BIOS_MIN_BYTES..=PS2_BIOS_MAX_BYTES).contains(&size) {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        if parse_ps2_bios(&bytes).is_none() {
            continue; // .ROM1 / .DIFF sized like a BIOS but without ROMVER
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_lowercase();
        std::fs::write(target.join(&stem).with_extension("bin"), bytes).unwrap();
        stems.push(stem);
    }
    if stems.is_empty() {
        return Err(NEEDS_BIOS);
    }
    Ok(stems)
}
