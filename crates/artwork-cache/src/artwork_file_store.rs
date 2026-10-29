//! The on-disk cache: `<artwork_dir>/<game_id>-cover.jpg`,
//! `<game_id>-hero.png` and `index/` (thumbnail index + ETag). Files appear
//! only via `*.tmp` + rename, so a crash or cancel never leaves a half file
//! under a final name.
//!
//! Files are keyed by the DB game id. `games.id` is `INTEGER PRIMARY KEY`
//! without AUTOINCREMENT, so SQLite may reuse an id after a delete; nothing
//! deletes games today, but a future delete must remove that game's artwork
//! files first or a new game could inherit them.

use std::path::Path;
use std::time::Duration;

use crate::ArtworkError;

/// Sub-directory for the thumbnail index; survives `clear_artwork_cache`.
pub(crate) const INDEX_DIR: &str = "index";
const TMP_SUFFIX: &str = ".tmp";
/// Antivirus / indexer briefly holding the fresh tmp file is common on Windows.
const RENAME_ATTEMPTS: u32 = 3;
const RENAME_RETRY_DELAY: Duration = Duration::from_millis(50);

/// What the artwork cache currently holds (art files only, index excluded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ArtworkCacheSize {
    pub files: u64,
    pub bytes: u64,
}

/// Writes `bytes` to `<dir>/<file_name>` via `<file_name>.tmp` + rename,
/// replacing an existing file. The tmp file is removed on any failure.
pub(crate) fn write_atomically(
    dir: &Path,
    file_name: &str,
    bytes: &[u8],
) -> Result<(), ArtworkError> {
    std::fs::create_dir_all(dir)
        .map_err(|error| ArtworkError::io(format!("cannot create {}", dir.display()), error))?;
    let final_path = dir.join(file_name);
    let tmp_path = dir.join(format!("{file_name}{TMP_SUFFIX}"));
    let result =
        write_synced(&tmp_path, bytes).and_then(|()| rename_with_retry(&tmp_path, &final_path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path); // best effort: no stray tmp file
    }
    result
        .map_err(|error| ArtworkError::io(format!("cannot write {}", final_path.display()), error))
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// `rename` with a short retry on Windows sharing violations
/// (ERROR_ACCESS_DENIED 5 / ERROR_SHARING_VIOLATION 32).
fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempt = 1;
    loop {
        match std::fs::rename(from, to) {
            Err(error)
                if attempt < RENAME_ATTEMPTS
                    && cfg!(windows)
                    && matches!(error.raw_os_error(), Some(5) | Some(32)) =>
            {
                attempt += 1;
                std::thread::sleep(RENAME_RETRY_DELAY);
            }
            other => return other,
        }
    }
}

/// Removes `*.tmp` leftovers (crash or kill mid-write) from the artwork dir
/// and its index dir; run at startup before any job. Returns how many went.
pub fn sweep_orphan_tmp(artwork_dir: &Path) -> usize {
    [artwork_dir.to_path_buf(), artwork_dir.join(INDEX_DIR)]
        .iter()
        .flat_map(|dir| regular_files(dir))
        .filter(|path| path.to_string_lossy().ends_with(TMP_SUFFIX))
        .filter(|path| std::fs::remove_file(path).is_ok())
        .count()
}

/// Number and total size of cached art files (index and `*.tmp` excluded).
/// A missing directory is an empty cache.
pub fn artwork_cache_size(artwork_dir: &Path) -> ArtworkCacheSize {
    regular_files(artwork_dir)
        .filter(|path| !path.to_string_lossy().ends_with(TMP_SUFFIX))
        .filter_map(|path| path.metadata().ok())
        .fold(ArtworkCacheSize::default(), |size, meta| ArtworkCacheSize {
            files: size.files + 1,
            bytes: size.bytes + meta.len(),
        })
}

/// Deletes every cached art file (and tmp leftovers). The thumbnail index is
/// kept: it is not art and re-fetching it costs GitHub API quota. Returns the
/// number of files removed; the first failure aborts with an error.
pub fn clear_artwork_cache(artwork_dir: &Path) -> Result<u64, ArtworkError> {
    let mut removed = 0;
    for path in regular_files(artwork_dir) {
        std::fs::remove_file(&path).map_err(|error| {
            ArtworkError::io(format!("cannot delete {}", path.display()), error)
        })?;
        removed += 1;
    }
    Ok(removed)
}

/// Regular files directly inside `dir` (no recursion); unreadable → none.
fn regular_files(dir: &Path) -> impl Iterator<Item = std::path::PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_tmp() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-artwork-store-")
            .tempdir()
            .unwrap();
        let dir = temp.path().join("artwork"); // not created yet: write_atomically makes it
        write_atomically(&dir, "5-cover.jpg", b"old").unwrap();
        write_atomically(&dir, "5-cover.jpg", b"new").unwrap();
        assert_eq!(std::fs::read(dir.join("5-cover.jpg")).unwrap(), b"new");
        assert!(!dir.join("5-cover.jpg.tmp").exists());
        assert_eq!(artwork_cache_size(&dir).files, 1);
    }
}
