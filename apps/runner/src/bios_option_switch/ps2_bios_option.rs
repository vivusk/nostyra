//! `pcsx2_bios` for PS2 cores (`FirmwareRule::Ps2Romdir`): the runner always names
//! the dump to boot — PCEE2 would otherwise take the first file by name and LRPS2
//! refuses to load (M18 spike step 4) — with the shared picker, so the file is the
//! one Settings → BIOS shows for the disc's region. The BIOS folder is scanned once
//! per launch (every 4–8 MiB file is read for its ROMDIR): `scan_report` memoises the
//! last folder's report, keyed by folder so unit tests on different temp dirs in one
//! process never see each other's files.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use disc_image::iso9660_root::PrimaryVolume;
use disc_image::{
    boot2_serial, disc_video_mode, is_m3u_ext, open_data_track, playlist_entries, system_cnf,
};
use system_catalog::{
    firmware_dir, ps2_bios_pick, scan_firmware, CoreSpec, FirmwareReport, FirmwareRule, SystemSpec,
};

static LAST: Mutex<Option<(PathBuf, FirmwareReport)>> = Mutex::new(None);

/// The core's BIOS report, scanned once per folder per process.
pub fn scan_report(system_dir: &Path, core: &CoreSpec) -> FirmwareReport {
    let folder = firmware_dir(system_dir, core);
    let mut last = LAST.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((scanned, report)) = last.as_ref() {
        if *scanned == folder {
            return report.clone();
        }
    }
    let report = scan_firmware(system_dir, core);
    *last = Some((folder, report.clone()));
    report
}

/// The `(option, file name)` answer for `rom` on `core`, logged once; `None` for
/// cores without the rule or without any valid dump (`prepare_core` refused before).
pub fn option(
    system: &SystemSpec,
    system_dir: &Path,
    core: &CoreSpec,
    rom: &Path,
) -> Option<(String, String)> {
    let FirmwareRule::Ps2Romdir { option, .. } = core.firmware_rule else {
        return None;
    };
    let report = scan_report(system_dir, core);
    let region = disc_region(rom);
    let chosen = ps2_bios_pick::choose(&report, region)?;
    tracing::info!(
        "{} BIOS: {} ({})",
        system.display_name,
        chosen.file_name,
        chosen.label.as_deref().unwrap_or("?")
    );
    Some((option.to_owned(), chosen.file_name.clone()))
}

/// Region the disc wants its BIOS for (`VMODE`, then the serial prefix); the first
/// disc of an `.m3u`. `None` when the disc cannot tell (the picker then takes the
/// newest dump).
fn disc_region(rom: &Path) -> Option<&'static str> {
    let ext = rom.extension().and_then(|e| e.to_str()).unwrap_or("");
    let disc = if is_m3u_ext(ext) {
        playlist_entries(rom).ok()?.into_iter().next()?
    } else {
        rom.to_path_buf()
    };
    let mut reader = open_data_track(&disc).ok()?;
    let pvd = PrimaryVolume::read(reader.as_mut()).ok()?;
    let cnf = system_cnf(reader.as_mut(), &pvd).ok()??;
    ps2_bios_pick::disc_region(disc_video_mode(&cnf), boot2_serial(&cnf).as_deref())
}

/// `pub(crate)`: `ps2_session_tests` shares `memo_lock` (one memo slot per process).
#[cfg(test)]
#[path = "ps2_bios_option_tests.rs"]
pub(crate) mod tests;
