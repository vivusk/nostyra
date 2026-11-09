//! Local backups of the losing side of a sync:
//! `<data>/cloud/backups/<unit key>/<YYMMDD-HHMMSS>-<side>/<path>` (`local` = this
//! PC's file replaced or deleted by a pull, `cloud` = the cloud copy overwritten by
//! "Keep this PC"). Never inside the synced folder; same volume as the saves, so a
//! backup is a rename. The 5 newest backup folders per unit are kept.

use std::io;
use std::path::{Path, PathBuf};

use crate::content_hash::sha256_file;
use crate::sync_staging::{cloud_unit_dir, join_rel, move_file, FsOp};

/// Backup folders kept per unit.
pub const BACKUPS_KEPT_PER_UNIT: usize = 5;

/// Which side a backup folder holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupSide {
    Local,
    Cloud,
}

impl BackupSide {
    fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }
}

/// `<data>/cloud/backups/<unit key>/`.
pub fn unit_backups_dir(data_root: &Path, unit_key: &str) -> PathBuf {
    cloud_unit_dir(data_root, "backups", unit_key)
}

/// One backup folder being filled (created on the first file).
#[derive(Debug, Clone)]
pub(crate) struct BackupFolder {
    dir: PathBuf,
}

impl BackupFolder {
    /// A fresh `<YYMMDD-HHMMSS>-<side>` folder name (`-02`, `-03`… when taken).
    pub(crate) fn new(data_root: &Path, unit_key: &str, side: BackupSide) -> Self {
        let parent = unit_backups_dir(data_root, unit_key);
        let stamp = chrono::Local::now().format("%y%m%d-%H%M%S");
        let base = format!("{stamp}-{}", side.label());
        let mut dir = parent.join(&base);
        let mut n = 2;
        while dir.exists() {
            // Zero-padded so names keep sorting by time within one second.
            dir = parent.join(format!("{base}-{n:02}"));
            n += 1;
        }
        Self { dir }
    }

    /// Moves the local file at `from` into the backup as `rel`.
    pub(crate) fn take(&self, from: &Path, rel: &str) -> io::Result<FsOp> {
        move_file(from, &self.path_of(rel))
    }

    /// The folder itself (exists once a file is in it).
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where `rel` lands in this backup.
    pub(crate) fn path_of(&self, rel: &str) -> PathBuf {
        join_rel(&self.dir, rel)
    }
}

/// Whether a backup folder of `side` already holds `rel` with content `sha256`
/// (a push retried after a failure must not back up the same cloud copy again).
pub(crate) fn holds_backup(
    data_root: &Path,
    unit_key: &str,
    side: BackupSide,
    rel: &str,
    sha256: &str,
) -> bool {
    let Ok(read) = std::fs::read_dir(unit_backups_dir(data_root, unit_key)) else {
        return false;
    };
    let tag = format!("-{}", side.label());
    read.filter_map(Result::ok).any(|entry| {
        let name = entry.file_name().to_string_lossy().into_owned();
        (name.ends_with(&tag) || name.contains(&format!("{tag}-")))
            && sha256_file(&join_rel(&entry.path(), rel)).is_ok_and(|sha| sha == sha256)
    })
}

/// Deletes all but the [`BACKUPS_KEPT_PER_UNIT`] newest backup folders of a unit
/// (names sort by time). `keep` (the folder just filled) is never deleted, even when
/// the clock went back and it sorts oldest. Errors are logged: pruning never fails a
/// sync.
pub(crate) fn prune_backups(data_root: &Path, unit_key: &str, keep: &Path) {
    let parent = unit_backups_dir(data_root, unit_key);
    let Ok(read) = std::fs::read_dir(&parent) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .map(|entry| entry.path())
        .collect();
    let kept = usize::from(dirs.iter().any(|dir| dir == keep));
    dirs.retain(|dir| dir != keep);
    dirs.sort();
    let excess = (dirs.len() + kept).saturating_sub(BACKUPS_KEPT_PER_UNIT);
    for dir in dirs.into_iter().take(excess) {
        if let Err(err) = std::fs::remove_dir_all(&dir) {
            tracing::warn!(target: "cloud_sync", "cannot prune backup {}: {err}", dir.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backups_are_moves_and_only_five_folders_stay() {
        let dir = tempfile::tempdir().unwrap();
        let key = "gba/abcd";
        let parent = unit_backups_dir(dir.path(), key);
        for i in 0..6 {
            std::fs::create_dir_all(parent.join(format!("26110{i}-000000-local"))).unwrap();
        }
        let src = dir.path().join("save.srm");
        std::fs::write(&src, b"x").unwrap();
        let backup = BackupFolder::new(dir.path(), key, BackupSide::Local);
        assert_eq!(backup.take(&src, "save.srm").unwrap(), FsOp::Cheap);
        assert!(!src.exists() && backup.path_of("save.srm").exists());
        prune_backups(dir.path(), key, backup.dir());
        let mut left: Vec<_> = std::fs::read_dir(&parent)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left.len(), BACKUPS_KEPT_PER_UNIT);
        assert!(!left.contains(&"261100-000000-local".to_string()));
        assert!(backup.path_of("save.srm").exists(), "newest kept");
    }

    #[test]
    fn prune_never_deletes_the_folder_just_filled_after_a_clock_step_back() {
        let dir = tempfile::tempdir().unwrap();
        let key = "gba/abcd";
        let parent = unit_backups_dir(dir.path(), key);
        for i in 0..5 {
            std::fs::create_dir_all(parent.join(format!("99110{i}-000000-local"))).unwrap();
        }
        let just_made = parent.join("000101-000000-local"); // clock went back
        std::fs::create_dir_all(&just_made).unwrap();
        prune_backups(dir.path(), key, &just_made);
        assert!(just_made.exists(), "the new backup survives");
        assert_eq!(
            std::fs::read_dir(&parent).unwrap().count(),
            BACKUPS_KEPT_PER_UNIT
        );
        assert!(
            !parent.join("991100-000000-local").exists(),
            "oldest other pruned"
        );
    }
}
