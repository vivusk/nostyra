//! File moves of the executor: the per-unit staging folder
//! `<data>/cloud/staging/<unit key>/{pull,push}/` (cleared at the start of every
//! unit sync: leftovers of a crash), path safety for paths read from a manifest, and
//! the cheap file operations of the `Applying` phase (rename / hard link, with a copy
//! fallback that is counted so logs and tests see it). Manifest path safety lives in
//! `manifest_path_safety` (re-exported here for the executor).

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Renames retried this often (antivirus / indexer holding the file on Windows).
const RENAME_TRIES: u32 = 3;
const RENAME_RETRY_DELAY: Duration = Duration::from_millis(100);

/// How a file got to its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FsOp {
    /// Same-volume rename / hard link (ms-scale).
    Cheap,
    /// Byte copy fallback (cross-volume, FAT, …).
    Copied,
}

pub use crate::manifest_path_safety::is_safe_relative_path;

/// `dir` joined with a safe relative `/` path.
pub(crate) fn join_rel(dir: &Path, rel: &str) -> PathBuf {
    rel.split('/')
        .fold(dir.to_path_buf(), |path, seg| path.join(seg))
}

/// `<data>/cloud/<kind>/<unit key>` (unit key segments are plain names).
pub(crate) fn cloud_unit_dir(data_root: &Path, kind: &str, unit_key: &str) -> PathBuf {
    join_rel(&data_root.join("cloud").join(kind), unit_key)
}

/// The staging folder of one unit.
#[derive(Debug, Clone)]
pub(crate) struct UnitStaging {
    root: PathBuf,
}

impl UnitStaging {
    /// Clears and recreates `<data>/cloud/staging/<unit key>/`.
    pub(crate) fn reset(data_root: &Path, unit_key: &str) -> io::Result<Self> {
        let staging = Self {
            root: cloud_unit_dir(data_root, "staging", unit_key),
        };
        staging.discard()?;
        std::fs::create_dir_all(&staging.root)?;
        Ok(staging)
    }

    /// Where a downloaded file waits before the apply pass.
    pub(crate) fn pull_path(&self, rel: &str) -> PathBuf {
        join_rel(&self.root.join("pull"), rel)
    }

    /// Where a local file is linked before upload.
    pub(crate) fn push_path(&self, rel: &str) -> PathBuf {
        join_rel(&self.root.join("push"), rel)
    }

    /// Removes the whole staging folder (already gone = ok).
    pub(crate) fn discard(&self) -> io::Result<()> {
        match std::fs::remove_dir_all(&self.root) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }
}

/// Moves `from` to `to` (parents created): rename, retried; when renaming keeps
/// failing (cross-volume), copy + delete.
pub(crate) fn move_file(from: &Path, to: &Path) -> io::Result<FsOp> {
    create_parent(to)?;
    let mut last = None;
    for attempt in 0..RENAME_TRIES {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(FsOp::Cheap),
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(err),
            Err(err) => last = Some(err),
        }
        if attempt + 1 < RENAME_TRIES {
            std::thread::sleep(RENAME_RETRY_DELAY);
        }
    }
    if let Some(err) = &last {
        tracing::debug!(target: "cloud_sync", "rename failed ({err}), copying {}", from.display());
    }
    std::fs::copy(from, to)?;
    if let Err(err) = std::fs::remove_file(from) {
        let _ = std::fs::remove_file(to); // keep exactly one copy
        return Err(err);
    }
    Ok(FsOp::Copied)
}

/// Hard-links `from` at `to` (parents created), copy when linking fails (FAT,
/// cross-volume). A link shares the bytes: an in-place write to the original after
/// this shows up in the upload hash and aborts the push.
pub(crate) fn link_or_copy(from: &Path, to: &Path) -> io::Result<FsOp> {
    create_parent(to)?;
    match std::fs::hard_link(from, to) {
        Ok(()) => Ok(FsOp::Cheap),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(err),
        Err(_) => std::fs::copy(from, to).map(|_| FsOp::Copied),
    }
}

fn create_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) => std::fs::create_dir_all(parent),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_clears_leftovers_and_moves_are_cheap_on_one_volume() {
        let dir = tempfile::tempdir().unwrap();
        let staging = UnitStaging::reset(dir.path(), "snes/ab").unwrap();
        let left = staging.pull_path("x/old.srm");
        std::fs::create_dir_all(left.parent().unwrap()).unwrap();
        std::fs::write(&left, b"old").unwrap();
        let staging = UnitStaging::reset(dir.path(), "snes/ab").unwrap();
        assert!(!left.exists());

        let src = dir.path().join("a.srm");
        std::fs::write(&src, b"1").unwrap();
        let linked = staging.push_path("a.srm");
        assert_eq!(link_or_copy(&src, &linked).unwrap(), FsOp::Cheap);
        let moved = dir.path().join("deep/b.srm");
        assert_eq!(move_file(&src, &moved).unwrap(), FsOp::Cheap);
        assert!(!src.exists() && moved.exists() && linked.exists());
        assert!(move_file(&src, &moved).is_err(), "missing source");
    }
}
