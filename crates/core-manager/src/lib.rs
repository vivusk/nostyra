//! Downloads libretro cores from the libretro buildbot and installs them
//! atomically into `<data>/cores/`.
//!
//! Cores are never bundled (Snes9x is non-commercial), so the launcher fetches
//! them on first run. The crate is DB-free and never loads a core itself: the
//! caller injects a probe (runner `--probe-core`) and records the returned
//! [`InstalledCore`].

#![deny(unsafe_code)]

mod buildbot_core_catalog;
mod core_archive_installer;
mod core_assets_installer;
mod core_downloader;
mod core_manager_error;
mod installed_core;

/// Largest core archive or extracted library we accept (snes9x ~3–6 MB, mesen ~4 MB).
pub const MAX_CORE_BYTES: u64 = 50 * 1024 * 1024;

pub use buildbot_core_catalog::{
    buildbot_core_source, current_platform_core_source, BuildbotCoreSource, BUILDBOT_BASE_URL,
};
pub use core_archive_installer::install_core_from_archive;
pub use core_assets_installer::{
    core_assets_url, install_core_with_assets, stage_core_assets, StagedCoreAssets,
    BUILDBOT_ASSETS_ROOT_URL, MAX_ASSETS_BYTES,
};
pub use core_downloader::install_core;
pub use core_manager_error::CoreManagerError;
pub use installed_core::{installed_core, CoreInfo, InstalledCore};
