//! Exit codes of the launcher contract (ipc-protocol's, as `u8` for `ExitCode`) and
//! the ROM file check (system + size limits from the registry, DS dumps the core
//! cannot play: `nds_rom_check`) before it is read.

use std::path::{Path, PathBuf};

use libretro_host::HostError;
use system_catalog::{ContentProbe, DiscProbe, SystemSpec};

use crate::core_system_assets::MissingCoreAssets;

mod nds_rom_check;

pub const EXIT_CORE_LOAD: u8 = ipc_protocol::EXIT_CORE_LOAD as u8;
pub const EXIT_ROM_LOAD: u8 = ipc_protocol::EXIT_ROM_LOAD as u8;
pub const EXIT_RUNTIME: u8 = ipc_protocol::EXIT_RUNTIME as u8;
pub const EXIT_GRAPHICS: u8 = ipc_protocol::EXIT_GRAPHICS as u8;

#[derive(Debug, thiserror::Error)]
pub enum RomValidationError {
    #[error("ROM not found or not a file: {0}")]
    NotAFile(PathBuf),
    #[error("cannot tell which system {path} is for: {reason} (pass --system)")]
    Unclassified { path: PathBuf, reason: String },
    #[error("ROM is {size} bytes, larger than the {max}-byte limit: {path}")]
    TooLarge { path: PathBuf, size: u64, max: u64 },
    #[error("ROM is {size} bytes, too small to be a valid image (min {min}): {path}")]
    TooSmall { path: PathBuf, size: u64, min: u64 },
    /// A disc game the core would fail on ("track file 'X' not found — …"), same text
    /// as the library scan problem.
    #[error("{reason}: {path}")]
    BrokenDisc { path: PathBuf, reason: String },
    /// A dump the core cannot play here (encrypted or DSi-only DS game).
    #[error("{reason}: {path}")]
    Unplayable { path: PathBuf, reason: &'static str },
}

/// Rejects missing/non-file ROMs, decides the system (`system` id from `--system`,
/// else extension + content like the library scanner) and applies its size limits
/// (the same ones the scanner uses, so a listed game never fails on size). The size
/// floor is the only cheap "not a ROM" check for cores that accept any data.
pub fn validate_rom(
    path: &Path,
    system: Option<&str>,
) -> Result<&'static SystemSpec, RomValidationError> {
    let meta = std::fs::metadata(path).map_err(|_| RomValidationError::NotAFile(path.into()))?;
    if !meta.is_file() {
        return Err(RomValidationError::NotAFile(path.into()));
    }
    let unclassified = |reason: String| RomValidationError::Unclassified {
        path: path.into(),
        reason,
    };
    let spec = match system {
        Some(id) => system_catalog::system(id)
            .ok_or_else(|| unclassified(format!("unknown system id \"{id}\"")))?,
        None => system_catalog::classify_path(path, &disc_image::DiscImageProbe)
            .map_err(|err| unclassified(err.to_string()))?,
    };
    // Re-check discs named by `--system` (the classifier already probed otherwise):
    // a track deleted or renamed since the scan fails here with a readable reason.
    if let (Some(_), ContentProbe::Disc(signature)) = (system, spec.probe) {
        disc_image::DiscImageProbe
            .check(signature, path)
            .map_err(|reason| RomValidationError::BrokenDisc {
                path: path.into(),
                reason,
            })?;
    }
    if meta.len() > spec.max_bytes {
        return Err(RomValidationError::TooLarge {
            path: path.into(),
            size: meta.len(),
            max: spec.max_bytes,
        });
    }
    if meta.len() < spec.min_bytes {
        return Err(RomValidationError::TooSmall {
            path: path.into(),
            size: meta.len(),
            min: spec.min_bytes,
        });
    }
    if spec.probe == ContentProbe::NdsHeader {
        if let Some(reason) = nds_rom_check::check_nds_file(path) {
            return Err(RomValidationError::Unplayable {
                path: path.into(),
                reason,
            });
        }
    }
    Ok(spec)
}

/// Maps an error chain to the process exit code.
pub fn exit_code_for(err: &anyhow::Error) -> u8 {
    for cause in err.chain() {
        if cause.downcast_ref::<RomValidationError>().is_some() {
            return EXIT_ROM_LOAD;
        }
        if cause.downcast_ref::<MissingCoreAssets>().is_some() {
            return EXIT_CORE_LOAD; // the core is incomplete: re-download it
        }
        if let Some(host) = cause.downcast_ref::<HostError>() {
            return match host {
                e if e.is_core_error() => EXIT_CORE_LOAD,
                HostError::ReadRom { .. }
                | HostError::StageContent { .. }
                | HostError::LoadGameRejected(_) => EXIT_ROM_LOAD,
                HostError::HwRenderUnavailable(_) => EXIT_GRAPHICS,
                _ => EXIT_RUNTIME,
            };
        }
    }
    EXIT_RUNTIME
}

#[cfg(test)]
#[path = "runner_exit_codes_tests.rs"]
mod tests;
