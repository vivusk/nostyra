//! `scan_firmware` per [`FirmwareRule`]: the known-files rule (PCSX ReARMed, Flycast;
//! moved here verbatim from `firmware_status.rs`) and the PS2 ROMDIR rule (every
//! 4–8 MiB regular file whose bytes parse; sidecars `.rom1 / .rom2 / .erom / .nvm /
//! .mec` never pass the size filter or the parse).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::core_specs::{CoreSpec, FirmwareRule};
use crate::firmware_status::{
    firmware_dir, is_valid_bios, BiosFileStatus, FirmwareReport, BIOS_HEAD_BYTES,
};
use crate::ps2_bios_romdir::{parse_ps2_bios, PS2_BIOS_MAX_BYTES, PS2_BIOS_MIN_BYTES};

/// Scans `core`'s BIOS folder with its rule (missing folder = no files).
pub fn scan_firmware(system_dir: &Path, core: &CoreSpec) -> FirmwareReport {
    let dir = firmware_dir(system_dir, core);
    match core.firmware_rule {
        FirmwareRule::KnownFiles => scan_known_files(&dir, core),
        FirmwareRule::Ps2Romdir { .. } => scan_ps2_romdir(&dir),
    }
}

fn scan_known_files(dir: &Path, core: &CoreSpec) -> FirmwareReport {
    let switch = core.bios_switch.is_some();
    let mut files: Vec<BiosFileStatus> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry
                .metadata()
                .is_ok_and(|m| m.is_file() && core.firmware.iter().any(|f| f.size == m.len()))
        })
        .filter_map(|entry| {
            let file_name = entry.file_name().to_str()?.to_owned();
            let known = core
                .firmware
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(&file_name));
            if switch && known.is_none() {
                return None; // such cores read their known names only
            }
            let valid = switch
                || read_at(&entry.path(), 0, BIOS_HEAD_BYTES)
                    .is_some_and(|head| is_valid_bios(&head));
            let region = known.map_or_else(|| guess_region(&entry.path()), |k| k.region);
            Some(BiosFileStatus {
                file_name,
                known,
                region,
                valid,
                label: None,
                ps2_version: None,
            })
        })
        .collect();
    let order = |f: &BiosFileStatus| {
        let rank = f
            .known
            .and_then(|k| core.firmware.iter().position(|x| x.name == k.name));
        (rank.unwrap_or(usize::MAX), f.file_name.to_ascii_lowercase())
    };
    files.sort_by_key(order);
    FirmwareReport { files }
}

/// Every 4–8 MiB regular file with a `ROMVER` entry, by name; each is read once.
fn scan_ps2_romdir(dir: &Path) -> FirmwareReport {
    let mut files: Vec<BiosFileStatus> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry.metadata().is_ok_and(|m| {
                m.is_file() && (PS2_BIOS_MIN_BYTES..=PS2_BIOS_MAX_BYTES).contains(&m.len())
            })
        })
        .filter_map(|entry| {
            let file_name = entry.file_name().to_str()?.to_owned();
            let version = parse_ps2_bios(&std::fs::read(entry.path()).ok()?)?;
            Some(BiosFileStatus {
                file_name,
                known: None,
                region: version.region_code(),
                valid: true,
                label: Some(version.label()),
                ps2_version: Some(version),
            })
        })
        .collect();
    files.sort_by_key(|f| f.file_name.to_ascii_lowercase());
    FirmwareReport { files }
}

/// Region marker the PlayStation core reads from an image it does not know by name.
fn guess_region(path: &Path) -> &'static str {
    match read_at(path, 0x7FF51, 2).as_deref() {
        Some(b" E") => "EU",
        Some(b" J") => "JP",
        _ => "US",
    }
}

fn read_at(path: &Path, offset: u64, len: usize) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = Vec::with_capacity(len);
    file.take(len as u64).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

#[cfg(test)]
#[path = "firmware_status_ps2_tests.rs"]
mod ps2_tests;
