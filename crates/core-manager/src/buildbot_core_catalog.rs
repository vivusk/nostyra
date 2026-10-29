//! Where the libretro buildbot publishes each core for each OS/arch.
//!
//! Mirrors `scripts/fetch-<core>-core.{sh,ps1}`: if the buildbot layout moves,
//! change [`BUILDBOT_BASE_URL`] and the platform table here and in the scripts.
//! Which cores exist (and their library file stems) comes from the system registry
//! (`system_catalog::CORES`).

use crate::CoreManagerError;

/// Root of the nightly buildbot tree; single place to change if it moves.
pub const BUILDBOT_BASE_URL: &str = "https://buildbot.libretro.com/nightly";

/// (Rust `std::env::consts::OS`, `ARCH`) → (buildbot platform path, library extension).
const PLATFORMS: &[(&str, &str, &str, &str)] = &[
    ("windows", "x86_64", "windows/x86_64", "dll"),
    ("linux", "x86_64", "linux/x86_64", "so"),
    ("macos", "aarch64", "apple/osx/arm64", "dylib"),
    ("macos", "x86_64", "apple/osx/x86_64", "dylib"),
];

/// What to fetch for one core on one platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildbotCoreSource {
    /// HTTPS URL of the zip archive.
    pub archive_url: String,
    /// The only entry extracted from the archive, also the installed file name.
    pub library_file_name: String,
}

/// Resolves the buildbot archive for `core_id` on an explicit OS/arch, so every
/// target can be tested from any host.
pub fn buildbot_core_source(
    core_id: &str,
    os: &str,
    arch: &str,
) -> Result<BuildbotCoreSource, CoreManagerError> {
    // Registry cores only: a typo or an untrusted id never becomes a buildbot URL.
    let core = system_catalog::core(core_id)
        .ok_or_else(|| CoreManagerError::UnknownCore(core_id.to_owned()))?;
    let (_, _, platform, extension) = PLATFORMS
        .iter()
        .find(|(p_os, p_arch, _, _)| *p_os == os && *p_arch == arch)
        .ok_or_else(|| CoreManagerError::UnsupportedPlatform {
            os: os.to_owned(),
            arch: arch.to_owned(),
        })?;
    let library_file_name = format!("{}.{extension}", core.library_stem);
    Ok(BuildbotCoreSource {
        archive_url: format!("{BUILDBOT_BASE_URL}/{platform}/latest/{library_file_name}.zip"),
        library_file_name,
    })
}

/// Resolves the buildbot archive for `core_id` on the platform this binary runs on.
pub fn current_platform_core_source(core_id: &str) -> Result<BuildbotCoreSource, CoreManagerError> {
    buildbot_core_source(core_id, std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
#[path = "buildbot_core_catalog_tests.rs"]
mod tests;
