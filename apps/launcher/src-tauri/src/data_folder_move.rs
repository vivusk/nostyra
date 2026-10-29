//! Moving Nostyra's own data from one data root to another (Settings → Data folder).
//! Only the entries Nostyra writes are touched, so a data root shared with other files
//! (e.g. `D:\Games`) keeps them. Copy first, delete after: a failed copy leaves the old
//! root complete, and the caller switches the location before deleting originals.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Top-level entries of a data root (`docs/system-architecture.md` § File locations).
/// The database's `-wal` / `-shm` files are gone once the connection is closed, but
/// are listed in case a crash left them. `logs` is deliberately absent: this run's log
/// is open, and the restart after the switch starts a fresh `logs/` in the new root
/// (the old logs stay behind with the old folder).
pub const DATA_ENTRIES: [&str; 13] = [
    "nostyra.db",
    "nostyra.db-wal",
    "nostyra.db-shm",
    "cores",
    "artwork",
    "saves",
    "config",
    "system",
    "sessions",
    "temp",
    "seed-library",
    // M15: RetroAchievements login, offline journal, snapshots; badge cache.
    "achievements",
    "cache",
];

/// Every file to move (relative to the root) and how many already exist at the target.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MovePlan {
    pub files: Vec<PathBuf>,
    pub conflicts: usize,
}

/// Lists the files under `from`'s data entries and counts those present under `to`.
pub fn plan_move(from: &Path, to: &Path) -> io::Result<MovePlan> {
    let mut files = Vec::new();
    for entry in DATA_ENTRIES {
        collect_files(from, Path::new(entry), &mut files)?;
    }
    let conflicts = files.iter().filter(|rel| to.join(rel).exists()).count();
    Ok(MovePlan { files, conflicts })
}

fn collect_files(root: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    let path = root.join(rel);
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if meta.is_dir() {
        for child in fs::read_dir(&path)? {
            collect_files(root, &rel.join(child?.file_name()), out)?;
        }
    } else if meta.is_file() {
        out.push(rel.to_path_buf());
    }
    Ok(())
}

/// Copies the plan's files to `to`. A file already at the target is replaced when
/// `overwrite`, else left alone (its original then stays in the old root). A file gone
/// since the plan (a temp file, an imported session) is skipped. Returns the files
/// copied; `progress(done, total)` follows every file. Stops at the first other error.
pub fn copy_planned_files(
    from: &Path,
    to: &Path,
    plan: &MovePlan,
    overwrite: bool,
    mut progress: impl FnMut(usize, usize),
) -> io::Result<Vec<PathBuf>> {
    let total = plan.files.len();
    let mut copied = Vec::with_capacity(total);
    for (index, rel) in plan.files.iter().enumerate() {
        let target = to.join(rel);
        let source = from.join(rel);
        if (overwrite || !target.exists()) && source.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source, &target)
                .map_err(|err| io::Error::new(err.kind(), format!("{}: {err}", rel.display())))?;
            copied.push(rel.clone());
        }
        progress(index + 1, total);
    }
    Ok(copied)
}

/// Deletes the copied originals, then the data-entry folders left empty. Best effort:
/// the data already lives in the new root, so a leftover only wastes space.
pub fn remove_moved_originals(from: &Path, copied: &[PathBuf]) {
    for rel in copied {
        if let Err(err) = fs::remove_file(from.join(rel)) {
            tracing::warn!(file = %rel.display(), %err, "moved file left in the old data folder");
        }
    }
    for entry in DATA_ENTRIES {
        remove_empty_dirs(&from.join(entry));
    }
}

/// Removes `dir` and its subfolders bottom-up while they are empty.
fn remove_empty_dirs(dir: &Path) {
    let Ok(children) = fs::read_dir(dir) else {
        return;
    };
    for child in children.flatten() {
        if child.file_type().is_ok_and(|t| t.is_dir()) {
            remove_empty_dirs(&child.path());
        }
    }
    let _ = fs::remove_dir(dir); // fails (kept) while not empty
}

/// Whether one folder is inside the other (or the same): moving would copy into itself.
pub fn folders_overlap(a: &Path, b: &Path) -> bool {
    let a = fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let b = fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    a.starts_with(&b) || b.starts_with(&a)
}

#[cfg(test)]
#[path = "data_folder_move_tests.rs"]
mod tests;
