//! What a unit's local folder holds right now: a recursive walk with relative `/`
//! paths, filtered by the scope's path rules. A file's sha256 is computed only when
//! its size or mtime differ from the base stat (otherwise the base sha is reused);
//! files over the size cap are never hashed (`sync_scope_maps` decides about them).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use walkdir::WalkDir;

use crate::base_manifest_store::{BaseManifest, LocalStat};
use crate::cloud_sync_error::CloudSyncError;
use crate::content_hash::sha256_file;
use crate::sync_scope::{is_excluded_dir, SyncScope};

/// One local file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub stat: LocalStat,
    /// Hex sha256; `None` for a file over the size cap (not hashed).
    pub sha256: Option<String>,
}

/// The unit folder's files by relative path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalSnapshot {
    pub files: BTreeMap<String, LocalFile>,
    /// How many files were hashed (the rest reused the base sha); for tests / logs.
    pub hashed: usize,
}

/// Walks `dir` (absent folder = empty snapshot). Symlinks and non-UTF-8 names are
/// skipped (logged); excluded folders (`User/Cache`) are not entered.
pub fn snapshot_local_dir(
    dir: &Path,
    base: Option<&BaseManifest>,
    scope: &SyncScope,
) -> Result<LocalSnapshot, CloudSyncError> {
    let mut snapshot = LocalSnapshot::default();
    if !dir.is_dir() {
        return Ok(snapshot);
    }
    let walker = WalkDir::new(dir).follow_links(false).into_iter();
    let walker = walker.filter_entry(|entry| {
        entry.depth() == 0
            || !entry.file_type().is_dir()
            || relative_path(dir, entry.path()).is_none_or(|rel| !is_excluded_dir(&rel))
    });
    for entry in walker {
        let entry = entry.map_err(|err| {
            let path = err.path().unwrap_or(dir).to_path_buf();
            CloudSyncError::io(path, err.into())
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(rel) = relative_path(dir, entry.path()) else {
            tracing::warn!(target: "cloud_sync", "skipping non-UTF-8 path {}", entry.path().display());
            continue;
        };
        if scope.excluded_path(&rel) {
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|err| CloudSyncError::io(entry.path(), err.into()))?;
        let stat = stat_of(&meta);
        let sha256 = if scope.over_cap(stat.size) {
            None
        } else if let Some(sha) = reusable_sha(base, &rel, stat) {
            Some(sha)
        } else {
            snapshot.hashed += 1;
            Some(sha256_file(entry.path()).map_err(|err| CloudSyncError::io(entry.path(), err))?)
        };
        snapshot.files.insert(rel, LocalFile { stat, sha256 });
    }
    Ok(snapshot)
}

/// Size + mtime (unix ms; 0 when the OS gives none) of a file.
pub(crate) fn stat_of(meta: &std::fs::Metadata) -> LocalStat {
    LocalStat {
        size: meta.len(),
        modified_ms: meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |age| u64::try_from(age.as_millis()).unwrap_or(u64::MAX)),
    }
}

/// The base sha when the base recorded the same size + mtime (mtime 0 = unknown,
/// never trusted).
fn reusable_sha(base: Option<&BaseManifest>, rel: &str, stat: LocalStat) -> Option<String> {
    let base = base?;
    let recorded = base.local.get(rel)?;
    (*recorded == stat && stat.modified_ms != 0).then(|| base.live_sha(rel).map(str::to_string))?
}

/// `path` relative to `dir` with `/` separators; `None` for non-UTF-8 names.
fn relative_path(dir: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(dir).ok()?;
    let parts: Option<Vec<&str>> = rel.components().map(|c| c.as_os_str().to_str()).collect();
    Some(parts?.join("/"))
}

#[cfg(test)]
#[path = "local_file_snapshot_tests.rs"]
mod tests;
