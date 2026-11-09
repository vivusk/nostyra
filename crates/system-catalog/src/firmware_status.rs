//! Which BIOS a core will use, read from its BIOS folder (`<system>/<subdir>`) the
//! way the core looks. PCSX ReARMed (M8 phase-00 spike): the known file names in the
//! core's order (any case), then any other 512 KiB file with the BIOS signature; a
//! disc of a region without its own BIOS still boots on another region's file.
//! Cores with a BIOS switch (Flycast, `dc/dc_boot.bin`): only the known names, at
//! their exact size; the md5 check that decides real vs HLE is the launcher's and
//! the runner's (std-only here: no hashing). Without any file the core's built-in
//! HLE BIOS runs. PS2 cores (`FirmwareRule::Ps2Romdir`, M18): any 4–8 MiB dump with
//! a ROMDIR `ROMVER` entry, labelled by version; no HLE — `ps2_bios_pick` chooses.
//! The per-rule folder walk lives in `firmware_scan_rule`.

use std::path::{Path, PathBuf};

use crate::core_specs::{CoreSpec, FirmwareFile, FirmwareRule};
use crate::ps2_bios_romdir::Ps2BiosVersion;

pub use crate::firmware_scan_rule::scan_firmware;

/// Size of every PlayStation BIOS image.
pub const BIOS_BYTES: u64 = 512 * 1024;
/// Region a file serves when the core accepts it for every region.
pub const ANY_REGION: &str = "ANY";
/// Regions the status view reports, in UI order.
pub const BIOS_REGIONS: [&str; 3] = ["JP", "US", "EU"];

/// One file of the BIOS folder that looks like a BIOS to the core's rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiosFileStatus {
    pub file_name: String,
    /// Registry entry with this name (case-insensitive); `None` under `Ps2Romdir`.
    pub known: Option<&'static FirmwareFile>,
    /// Region it boots: the registry's for a known name, else guessed from the image
    /// like the core does (`" E"` / `" J"` at 0x7FF51, otherwise US); PS2: `ROMVER`.
    pub region: &'static str,
    /// Size and signature check passed (the core skips invalid files).
    pub valid: bool,
    /// Human label of the dump ("v1.60 · USA · 2002-02-07"); PS2 only.
    pub label: Option<String>,
    /// Decoded `ROMVER` (comparable: the picker takes the newest); PS2 only.
    pub ps2_version: Option<Ps2BiosVersion>,
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
    /// Known names first in the core's order, then the rest by name (PS2: by name).
    pub files: Vec<BiosFileStatus>,
}

impl FirmwareReport {
    /// The files the core would boot.
    pub fn valid(&self) -> impl Iterator<Item = &BiosFileStatus> {
        self.files.iter().filter(|f| f.valid)
    }

    /// BIOS used for a disc of `region` (`"JP"`, `"US"`, `"EU"`) by a `KnownFiles`
    /// core: a valid file of that region (or any-region file), else any valid file,
    /// else HLE. `Ps2Romdir` cores use `ps2_bios_pick::choose` instead.
    pub fn choice_for(&self, region: &str) -> BiosChoice {
        let regional = self
            .valid()
            .find(|f| f.region == region || f.region == ANY_REGION);
        match regional.or_else(|| self.valid().next()) {
            Some(file) => BiosChoice::Real {
                file_name: file.file_name.clone(),
            },
            None => BiosChoice::Hle,
        }
    }
}

/// The folder `core` reads its BIOS from: `<system_dir>/<subdir>` (one subdir per
/// core; the rule's for PS2, the first firmware entry's otherwise).
pub fn firmware_dir(system_dir: &Path, core: &CoreSpec) -> PathBuf {
    let subdir = match core.firmware_rule {
        FirmwareRule::Ps2Romdir { subdir, .. } => subdir,
        FirmwareRule::KnownFiles => core.firmware.first().map_or("", |f| f.subdir),
    };
    system_dir.join(subdir)
}

/// Regions the status lists for `core`: JP / US / EU, or one [`ANY_REGION`] row when
/// every BIOS file of the core boots every region (PS2 dumps are regional).
pub fn bios_regions(core: &CoreSpec) -> &'static [&'static str] {
    let regional = matches!(core.firmware_rule, FirmwareRule::Ps2Romdir { .. })
        || core.firmware.iter().any(|f| f.region != ANY_REGION);
    if regional {
        &BIOS_REGIONS
    } else {
        &[ANY_REGION]
    }
}

/// The PlayStation core's signature check on the first bytes of a 512 KiB image: a
/// MIPS `lui t0` at byte 1, or "PS compatible" at 0x12C.
pub fn is_valid_bios(head: &[u8]) -> bool {
    head.get(1..5) == Some(&[0x00, 0x08, 0x3C, 0x3F][..])
        || head.get(0x12C..0x12C + 13) == Some(b"PS compatible")
}

/// Bytes [`is_valid_bios`] looks at.
pub const BIOS_HEAD_BYTES: usize = 0x140;

#[cfg(test)]
#[path = "firmware_status_tests.rs"]
mod tests;
