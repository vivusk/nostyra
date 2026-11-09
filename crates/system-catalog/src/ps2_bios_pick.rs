//! Which PS2 BIOS dump boots a disc — one answer for the launcher's Settings → BIOS
//! region rows and the runner's `pcsx2_bios` option, so both name the same file:
//! valid dumps of the disc's region (or any-region dumps) → else every valid dump →
//! the newest `ROMVER` → ties by file name (the report's order). The cores
//! themselves would take the first file by name (PCEE2 `auto`) or refuse to load
//! (LRPS2), which is why the runner always answers.

use crate::firmware_status::{BiosFileStatus, FirmwareReport, ANY_REGION};
use crate::ps2_video_mode::Ps2VideoMode;

/// The dump to boot, or `None` when the report has no valid file.
pub fn choose<'a>(
    report: &'a FirmwareReport,
    disc_region: Option<&str>,
) -> Option<&'a BiosFileStatus> {
    let regional = |f: &&BiosFileStatus| {
        disc_region.is_some_and(|region| f.region == region || f.region == ANY_REGION)
    };
    let newest = |candidates: &mut dyn Iterator<Item = &'a BiosFileStatus>| {
        // `min_by` keeps the first of equal elements: name order breaks version ties.
        candidates.min_by(|a, b| b.ps2_version.cmp(&a.ps2_version))
    };
    newest(&mut report.valid().filter(regional)).or_else(|| newest(&mut report.valid()))
}

/// Region a disc wants its BIOS for: PAL → `EU`; otherwise from the serial prefix
/// (`SLUS` / `SCUS` → `US`, Japanese prefixes → `JP`, `SLES` / `SCES` → `EU`);
/// `None` when neither tells.
pub fn disc_region(video_mode: Option<Ps2VideoMode>, serial: Option<&str>) -> Option<&'static str> {
    if video_mode == Some(Ps2VideoMode::Pal) {
        return Some("EU");
    }
    let prefix = serial?.get(..4)?.to_ascii_uppercase();
    match prefix.as_str() {
        "SLUS" | "SCUS" => Some("US"),
        "SLPS" | "SLPM" | "SCPS" | "SCAJ" | "SLKA" => Some("JP"),
        "SLES" | "SCES" => Some("EU"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "ps2_bios_pick_tests.rs"]
mod tests;
