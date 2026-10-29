//! Core system files (`CoreSpec::system_assets`, e.g. PPSSPP's fonts, dialog atlas and
//! compat table) in `<data>/system/<dir>/`, installed before the core loads from the
//! copy downloaded beside the core library: `<cores>/<dir>/` (Settings → Cores, or
//! `scripts/fetch-ppsspp-core.*` into the repo's `cores/`).
//!
//! Installed = every marker file present and the first marker equal by content to the
//! core's copy (a re-downloaded core with newer files is copied again; content, not
//! mtime: copies and zip extraction do not keep mtimes everywhere). Without a copy
//! beside the core, installed markers are enough. Copy = staged replace (copy to
//! `<dir>.tmp`, swap; leftovers of an interrupted run are cleaned first).

use std::io;
use std::path::{Path, PathBuf};

use config_layers::{remove_stale_staging, replace_dir, staging_dirs};
use system_catalog::{CoreAssets, CoreSpec};

/// The core's system files are neither installed nor beside the core: the launch
/// fails with exit 3 (`runner_exit_codes`), and this text is the error shown.
#[derive(Debug, thiserror::Error)]
#[error(
    "{display_name} system files are missing ({}). Re-download the core in Settings → Cores.",
    path.display()
)]
pub struct MissingCoreAssets {
    display_name: &'static str,
    path: PathBuf,
}

/// What [`ensure_core_assets`] found or did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetsState {
    /// Already installed and current.
    Present,
    /// Copied from beside the core just now.
    Installed,
}

/// Folder the `--core` library lives in: the launcher's `<data>/cores`, or the
/// repo's `cores/` for dev runs and tests (never derived from the data dir).
pub fn cores_dir_of(core_path: &Path) -> &Path {
    core_path.parent().unwrap_or(Path::new("."))
}

/// Makes sure `core`'s system files are in `system_dir`; `None` for cores without any.
pub fn ensure_core_assets(
    core: &CoreSpec,
    system_dir: &Path,
    cores_dir: &Path,
) -> Result<Option<AssetsState>, MissingCoreAssets> {
    let Some(assets) = core.system_assets else {
        return Ok(None);
    };
    let target = system_dir.join(assets.dir);
    let source = cores_dir.join(assets.dir);
    let installed = has_markers(&assets, &target);
    let source_ok = has_markers(&assets, &source);
    if installed && (!source_ok || same_first_marker(&assets, &source, &target)) {
        let _ = remove_stale_staging(&target); // best effort: old leftovers only
        return Ok(Some(AssetsState::Present));
    }
    let missing = || MissingCoreAssets {
        display_name: core.display_name,
        path: target.clone(),
    };
    if !source_ok {
        return Err(missing());
    }
    match install(&source, &target) {
        Ok(()) => {
            tracing::info!(from = %source.display(), to = %target.display(), "core system files installed");
            Ok(Some(AssetsState::Installed))
        }
        Err(error) if installed => {
            tracing::warn!(%error, "cannot update the core system files; using the installed ones");
            Ok(Some(AssetsState::Present))
        }
        Err(error) => {
            tracing::error!(%error, "cannot install the core system files");
            Err(missing())
        }
    }
}

fn has_markers(assets: &CoreAssets, dir: &Path) -> bool {
    assets
        .markers
        .iter()
        .all(|marker| dir.join(marker).is_file())
}

/// First marker byte-equal in both folders (sizes first: the common mismatch).
fn same_first_marker(assets: &CoreAssets, a: &Path, b: &Path) -> bool {
    let Some(marker) = assets.markers.first() else {
        return true;
    };
    let (a, b) = (a.join(marker), b.join(marker));
    let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
    if size(&a) != size(&b) {
        return false;
    }
    matches!((std::fs::read(&a), std::fs::read(&b)), (Ok(x), Ok(y)) if x == y)
}

fn install(source: &Path, target: &Path) -> io::Result<()> {
    remove_stale_staging(target)?;
    let (tmp, _) = staging_dirs(target);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let copied = copy_dir(source, &tmp).and_then(|()| replace_dir(&tmp, target));
    if copied.is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    copied
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "core_system_assets_tests.rs"]
mod tests;
