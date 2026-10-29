//! What an install produced, and the cheap "already installed?" check that
//! lets the first-run flow skip the download.

use std::path::{Path, PathBuf};

use crate::buildbot_core_catalog::current_platform_core_source;

/// Identity reported by the probe (libretro `retro_get_system_info`), so the
/// launcher can show and store which core build is installed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CoreInfo {
    /// Library name, e.g. "Snes9x".
    pub name: String,
    /// Library version string as reported by the core.
    pub version: String,
}

/// A core that passed the probe and was atomically moved into the cores dir.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledCore {
    /// Final library path inside the cores dir.
    pub path: PathBuf,
    /// Lower-case hex SHA256 of the library: nightlies are unpinned, so the
    /// hash is recorded for traceability instead of verified against a golden value.
    pub sha256: String,
    /// What the probe reported for this file.
    pub info: CoreInfo,
}

/// Path of `core_id`'s library in `cores_dir` if a file is already there.
///
/// Existence only (no probe, no hash): the launcher records the probe result
/// when it installs, so a second "ensure core" is a no-op. Unknown core ids
/// and unsupported platforms yield `None`.
pub fn installed_core(cores_dir: &Path, core_id: &str) -> Option<PathBuf> {
    let source = current_platform_core_source(core_id).ok()?;
    let path = cores_dir.join(source.library_file_name);
    path.is_file().then_some(path)
}
