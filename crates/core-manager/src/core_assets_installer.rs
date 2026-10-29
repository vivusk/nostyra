//! Core system files (`CoreSpec::system_assets`, e.g. PPSSPP's fonts, dialog atlas,
//! compat table): the buildbot's separate zip is extracted whole into a staging
//! folder before the core library is swapped in, then moved to `<cores>/<dir>/`.
//! The runner copies that folder into `<data>/system/<dir>/` before load.
//!
//! Every entry name goes through `enclosed_name` (no absolute or `..` paths: an
//! entry outside the target is a broken archive, the whole install fails) and the
//! uncompressed total is capped.

use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use config_layers::{remove_stale_staging, replace_dir, staging_dirs};
use http_download_util::copy_with_limit;
use system_catalog::CoreAssets;

use crate::core_archive_installer::install_core_from_archive;
use crate::{CoreInfo, CoreManagerError, InstalledCore};

/// Largest uncompressed total of one assets archive (PPSSPP: ~15 MB).
pub const MAX_ASSETS_BYTES: u64 = 64 * 1024 * 1024;

/// Buildbot root of the assets tree (`<root>/<archive_path>`); the nightly cores
/// live under `<root>/nightly`. Mirrored in `scripts/fetch-ppsspp-core.*`.
pub const BUILDBOT_ASSETS_ROOT_URL: &str = "https://buildbot.libretro.com";

/// Download URL of `assets`.
pub fn core_assets_url(assets: &CoreAssets) -> String {
    format!("{BUILDBOT_ASSETS_ROOT_URL}/{}", assets.archive_path)
}

/// Assets extracted and checked, waiting for the library install to finish.
#[derive(Debug)]
pub struct StagedCoreAssets {
    /// `<cores>/<dir>.tmp`: the extracted archive (holds `<dir>/`).
    root: PathBuf,
    /// The archive's top folder = the asset folder name.
    dir: &'static str,
    /// Final `<cores>/<dir>`.
    dest: PathBuf,
}

/// Offline install of a core plus its system files. The assets archive is
/// extracted and checked first, so a broken one fails the install before the
/// library (and the caller's DB record) is touched; then the library swap; then
/// the assets move into `<cores>/<dir>/`. A failure of that last move leaves the
/// new library with old or no assets: the runner then refuses to start with a
/// re-download hint (exit 3).
pub fn install_core_with_assets(
    core_archive: &[u8],
    assets: Option<(&CoreAssets, &[u8])>,
    core_id: &str,
    cores_dir: &Path,
    probe: impl FnOnce(&Path) -> Result<CoreInfo, String>,
    cancel: &AtomicBool,
) -> Result<InstalledCore, CoreManagerError> {
    let staged = match assets {
        Some((spec, archive)) => Some(stage_core_assets(archive, spec, cores_dir, cancel)?),
        None => None,
    };
    let installed = install_core_from_archive(core_archive, core_id, cores_dir, probe, cancel);
    let installed = match installed {
        Ok(installed) => installed,
        Err(error) => {
            if let Some(staged) = &staged {
                staged.discard();
            }
            return Err(error);
        }
    };
    if let Some(staged) = staged {
        let dir = staged.finish()?;
        tracing::info!(dir = %dir.display(), "installed core system files");
    }
    Ok(installed)
}

/// Extracts `archive` into `<cores>/<dir>.tmp/` and checks the markers under its
/// `<dir>/` folder (the buildbot zip's top folder). Nothing outside the staging
/// folder is touched; on error it is removed.
pub fn stage_core_assets(
    archive: &[u8],
    assets: &CoreAssets,
    cores_dir: &Path,
    cancel: &AtomicBool,
) -> Result<StagedCoreAssets, CoreManagerError> {
    let dest = cores_dir.join(assets.dir);
    remove_stale_staging(&dest).map_err(|e| CoreManagerError::io("cannot clean up", e))?;
    let (root, _) = staging_dirs(&dest);
    let staged = StagedCoreAssets {
        root,
        dir: assets.dir,
        dest,
    };
    let result = extract_all(archive, &staged.root, cancel).and_then(|()| {
        let missing = assets
            .markers
            .iter()
            .find(|marker| !staged.root.join(assets.dir).join(marker).is_file());
        match missing {
            Some(marker) => Err(CoreManagerError::AssetsArchive(format!(
                "{}/{marker} missing",
                assets.dir
            ))),
            None => Ok(()),
        }
    });
    match result {
        Ok(()) => Ok(staged),
        Err(error) => {
            staged.discard();
            Err(error)
        }
    }
}

impl StagedCoreAssets {
    /// Moves the staged `<dir>/` to `<cores>/<dir>` (replacing an older copy).
    pub fn finish(self) -> Result<PathBuf, CoreManagerError> {
        let result = replace_dir(&self.root.join(self.dir), &self.dest)
            .map_err(|e| CoreManagerError::io("cannot install the core system files", e));
        let _ = std::fs::remove_dir_all(&self.root); // best effort: now empty
        result.map(|()| self.dest)
    }

    /// Drops the staging folder (the install failed).
    pub fn discard(&self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn extract_all(archive: &[u8], root: &Path, cancel: &AtomicBool) -> Result<(), CoreManagerError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive))?;
    let mut budget = MAX_ASSETS_BYTES;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(relative) = entry.enclosed_name() else {
            let name = entry.name().map(|n| n.into_owned()).unwrap_or_default();
            return Err(CoreManagerError::AssetsArchive(format!(
                "unsafe entry {name}"
            )));
        };
        let path = root.join(relative);
        let context = format!("cannot write {}", path.display());
        if entry.is_dir() {
            std::fs::create_dir_all(&path).map_err(|e| CoreManagerError::io(&context, e))?;
            continue;
        }
        if entry.size() > budget {
            return Err(CoreManagerError::TooLarge {
                limit_bytes: MAX_ASSETS_BYTES,
            });
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreManagerError::io(&context, e))?;
        }
        let mut file = File::create(&path).map_err(|e| CoreManagerError::io(&context, e))?;
        // Capped again while streaming: the header size could lie.
        let written = copy_with_limit(&mut entry, &mut file, budget, cancel, |_| {}, &context)?;
        budget -= written;
    }
    Ok(())
}
