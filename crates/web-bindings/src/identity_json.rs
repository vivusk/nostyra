//! Scan-side logic for one file, on paths of the current content source (the web
//! worker installs its File System Access reads first): classify, identify + DAT
//! facts, single-ROM ZIPs and soft patches. Same functions as the desktop scanner.

use std::path::Path;

use disc_image::DiscImageProbe;
use rom_identity::serial_match_keys::serial_rule;
use rom_identity::{
    dat_facts, game_identity, match_dat, scan_zip, scanned_game_metadata, DatIndex, IdentityError,
    MetadataIndex, ZipScan,
};
use serde::Serialize;
use system_catalog::{classify_path, system, ClassifyError};

use crate::library_json::MetadataView;

/// A library row's identity facts (camelCase for TypeScript).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifiedGame {
    pub system: String,
    pub crc32: String,
    pub sha1: String,
    /// Hashed size (what DAT `size` refers to).
    pub size: u64,
    /// `[ID]` of the save folder / game config layer (first 8 hex of the sha1).
    pub content_id: Option<String>,
    pub title: String,
    pub dat_name: Option<String>,
    pub region: Option<String>,
    pub serial: Option<String>,
    /// Vendored metadat of the game (desktop rules); `None` = nothing known.
    pub metadata: Option<MetadataView>,
}

/// System id of the file at `path` (extension, size, content probe — the desktop
/// classifier); `Err` carries the reason shown in the scan report.
pub fn classify(path: &Path) -> Result<&'static str, String> {
    classify_path(path, &DiscImageProbe)
        .map(|spec| spec.id)
        .map_err(|err| err.to_string())
}

/// Builds a DAT index for `system_id` from the bundled DAT text.
pub fn load_dat(system_id: &str, text: &str) -> Result<DatIndex, String> {
    let spec = system(system_id).ok_or_else(|| format!("unknown system {system_id}"))?;
    Ok(DatIndex::from_text(text)?.with_serial_rule(serial_rule(spec)))
}

/// Hashes the game at `path` as `system_id`, matches it against `dat` (an empty
/// index when the DAT is not loaded: every game then needs review) and picks its
/// `metadata` record — the desktop scanner's row, field for field.
pub fn identify(
    path: &Path,
    system_id: &str,
    dat: &DatIndex,
    metadata: &MetadataIndex,
) -> Result<IdentifiedGame, String> {
    let spec = system(system_id).ok_or_else(|| format!("unknown system {system_id}"))?;
    let identity = game_identity(path, spec).map_err(|err| match err {
        IdentityError::ZipSystemMismatch => err.to_string(),
        IdentityError::Read(message) => message,
    })?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let matched = match_dat(dat, &identity);
    let facts = dat_facts(&identity, matched, stem);
    let record = scanned_game_metadata(metadata, &identity, matched, &facts);
    Ok(IdentifiedGame {
        metadata: record.map(|record| MetadataView::new(record, metadata)),
        system: spec.id.to_string(),
        content_id: ipc_protocol::content_id(&identity.hashes.sha1),
        crc32: identity.hashes.crc32,
        sha1: identity.hashes.sha1,
        size: identity.hashes.size,
        title: facts.title,
        dat_name: facts.dat_name,
        region: facts.region,
        serial: facts.serial,
    })
}

/// Scan verdict of one file (camelCase JSON, tagged by `kind`): its system, a quiet
/// skip (not a ROM of any system) or a problem the scan report lists — the desktop
/// scanner's policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScanVerdict {
    Ok { system: String },
    Skip,
    Problem { reason: String },
}

/// [`classify`] as a scan verdict: out of size, no candidate and "not a ROM" are
/// skipped quietly (`folder_scanner`), every other error is a problem.
pub fn classify_for_scan(path: &Path) -> ScanVerdict {
    match classify_path(path, &DiscImageProbe) {
        Ok(spec) => ScanVerdict::Ok {
            system: spec.id.to_string(),
        },
        Err(ClassifyError::OutOfSize | ClassifyError::NoCandidate | ClassifyError::NotRom(_)) => {
            ScanVerdict::Skip
        }
        Err(err) => ScanVerdict::Problem {
            reason: err.to_string(),
        },
    }
}

/// Scan verdict for a `.zip` among `allowed` system ids, without inflating the ROM.
pub fn zip_single_rom(path: &Path, allowed: &[String]) -> ScanVerdict {
    let specs: Vec<_> = allowed.iter().filter_map(|id| system(id)).collect();
    match scan_zip(path, &specs) {
        ZipScan::Rom(spec) => ScanVerdict::Ok {
            system: spec.id.to_string(),
        },
        ZipScan::Skip => ScanVerdict::Skip,
        ZipScan::Problem(reason) => ScanVerdict::Problem { reason },
    }
}

/// The ROM inside a single-ROM `.zip`: file name, system and bytes (what the player
/// hands to EmulatorJS; same rules as the desktop scan and runner).
pub fn read_zip_rom(path: &Path) -> Result<(String, &'static str, Vec<u8>), String> {
    let rom = rom_identity::read_rom_archive(path).map_err(|err| err.to_string())?;
    Ok((rom.name, rom.system.id, rom.bytes))
}

/// Applies an IPS / UPS / BPS soft patch to `rom` (format detected from the patch).
pub fn apply_patch(rom: &[u8], patch: &[u8]) -> Result<Vec<u8>, String> {
    rom_patch::apply_patch(rom, patch)
        .map(|(_, bytes)| bytes)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
#[path = "identity_json_tests.rs"]
mod tests;
