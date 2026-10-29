//! What rcheevos hashes to identify a game, and hashing with our readers.
//!
//! - Cartridges: the bytes the core received when a soft patch was applied (the
//!   patched game is what RA knows), else the file.
//! - Discs: always the path (rcheevos re-reads the image, a buffer is refused for
//!   `.gdi` / `.cso` / `.cdi` / `.pbp`), read through the Rust filereader and the
//!   `disc-image` cdreader (CHD / CSO / CDI / Redump GD cue).
//! - PS1 `.pbp`: rcheevos v12.5.0 cannot hash it (M15 D7) → unavailable. A PSP
//!   `.pbp` is hashed as a whole file (first 64 MiB, rcheevos' cap).

use std::path::{Path, PathBuf};

use system_catalog::{Media, SystemSpec};

use crate::ffi::{generate_hash, HashReaders};

/// RA console id of the PlayStation (its `.pbp` eboots are not hashable).
const PLAYSTATION_CONSOLE: u8 = 12;

/// Arguments for `AchievementClient::begin_identify_and_load_game`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashRequest {
    pub console_id: u32,
    pub path: PathBuf,
    /// Patched cartridge bytes (`None`: hash the file at `path`).
    pub data: Option<Vec<u8>>,
}

/// Why a game cannot get achievements before any server call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HashUnavailable {
    #[error("RetroAchievements does not support this system")]
    System,
    #[error("achievements are not available for this format")]
    Format,
}

/// The hash input for `path` on `spec`'s system; `patched` = the soft-patched
/// cartridge bytes handed to the core, if any (ignored for discs).
pub fn hash_request(
    spec: &SystemSpec,
    path: &Path,
    patched: Option<Vec<u8>>,
) -> Result<HashRequest, HashUnavailable> {
    let console = spec.ra_console_id.ok_or(HashUnavailable::System)?;
    let is_pbp = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(disc_image::PBP_EXT));
    if console == PLAYSTATION_CONSOLE && is_pbp {
        return Err(HashUnavailable::Format);
    }
    let data = match spec.media {
        Media::Cartridge => patched,
        Media::Disc => None,
    };
    Ok(HashRequest {
        console_id: u32::from(console),
        path: path.to_path_buf(),
        data,
    })
}

/// rcheevos' hash of `path` (or of `data`) for `console_id`, through our readers;
/// e.g. for a disc swap (rc_client's change-media path ignores per-client readers,
/// M15 D8). `None`: no hash (unsupported format, unreadable file, non-UTF-8 path).
pub fn content_hash(console_id: u32, path: &Path, data: Option<&[u8]>) -> Option<String> {
    generate_hash(console_id, path.to_str()?, data, HashReaders::Ours)
}

#[cfg(test)]
#[path = "content_hash_tests.rs"]
mod tests;
