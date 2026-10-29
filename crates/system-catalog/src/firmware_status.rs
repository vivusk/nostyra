//! Which BIOS a core will use, read from its BIOS folder (`<system>/<subdir>`) the
//! way the core looks. PCSX ReARMed (M8 phase-00 spike): the known file names in the
//! core's order (any case), then any other 512 KiB file with the BIOS signature; a
//! disc of a region without its own BIOS still boots on another region's file.
//! Cores with a BIOS switch (Flycast, `dc/dc_boot.bin`): only the known names, at
//! their exact size; the md5 check that decides real vs HLE is the launcher's and
//! the runner's (std-only here: no hashing). Without any file the core's built-in
//! HLE BIOS runs.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::core_specs::{CoreSpec, FirmwareFile};

/// Size of every PlayStation BIOS image.
pub const BIOS_BYTES: u64 = 512 * 1024;
/// Region a file serves when the core accepts it for every region.
pub const ANY_REGION: &str = "ANY";
/// Regions the status view reports, in UI order.
pub const BIOS_REGIONS: [&str; 3] = ["JP", "US", "EU"];

/// One file of the system folder that looks like a BIOS (512 KiB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiosFileStatus {
    pub file_name: String,
    /// Registry entry with this name (case-insensitive).
    pub known: Option<&'static FirmwareFile>,
    /// Region it boots: the registry's for a known name, else guessed from the image
    /// like the core does (`" E"` / `" J"` at 0x7FF51, otherwise US).
    pub region: &'static str,
    /// Size and signature check passed (the core skips invalid files).
    pub valid: bool,
}

/// What the core boots for one region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BiosChoice {
    /// A real BIOS file (`file_name` in the system folder).
    Real { file_name: String },
    /// The core's built-in HLE BIOS.
    Hle,
}

/// Status of a core's BIOS files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareReport {
    /// 512 KiB files, known names first in the core's order, then the rest by name.
    pub files: Vec<BiosFileStatus>,
}

impl FirmwareReport {
    /// BIOS used for a disc of `region` (`"JP"`, `"US"`, `"EU"`): a valid file of that
    /// region (or any-region file), else any valid file, else HLE.
    pub fn choice_for(&self, region: &str) -> BiosChoice {
        let valid = || self.files.iter().filter(|f| f.valid);
        let regional = valid().find(|f| f.region == region || f.region == ANY_REGION);
        match regional.or_else(|| valid().next()) {
            Some(file) => BiosChoice::Real {
                file_name: file.file_name.clone(),
            },
            None => BiosChoice::Hle,
        }
    }
}

/// The folder `core` reads its BIOS from: `<system_dir>/<subdir>` (one subdir per
/// core).
pub fn firmware_dir(system_dir: &Path, core: &CoreSpec) -> PathBuf {
    system_dir.join(core.firmware.first().map_or("", |f| f.subdir))
}

/// Regions the status lists for `core`: JP / US / EU, or one [`ANY_REGION`] row when
/// every BIOS file of the core boots every region.
pub fn bios_regions(core: &CoreSpec) -> &'static [&'static str] {
    if core.firmware.iter().all(|f| f.region == ANY_REGION) {
        &[ANY_REGION]
    } else {
        &BIOS_REGIONS
    }
}

/// Scans `core`'s BIOS folder for its BIOS files (missing folder = no files).
pub fn scan_firmware(system_dir: &Path, core: &CoreSpec) -> FirmwareReport {
    let switch = core.bios_switch.is_some();
    let mut files: Vec<BiosFileStatus> = std::fs::read_dir(firmware_dir(system_dir, core))
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

/// The core's signature check on the first bytes of a 512 KiB image: a MIPS
/// `lui t0` at byte 1, or "PS compatible" at 0x12C.
pub fn is_valid_bios(head: &[u8]) -> bool {
    head.get(1..5) == Some(&[0x00, 0x08, 0x3C, 0x3F][..])
        || head.get(0x12C..0x12C + 13) == Some(b"PS compatible")
}

/// Bytes [`is_valid_bios`] looks at.
pub const BIOS_HEAD_BYTES: usize = 0x140;

/// Region marker the core reads from an image it does not know by name.
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
#[path = "firmware_status_tests.rs"]
mod tests;
