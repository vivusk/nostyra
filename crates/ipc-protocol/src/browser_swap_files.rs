//! Chrome's File System Access swap files. The web edition writes every Data-folder
//! file atomically through `createWritable()`: Chrome writes `<name>.crswap` beside
//! the target and renames it on close. A tab closed mid-write leaves the `.crswap`
//! behind; it never holds data either app needs, so the desktop skips these files
//! and removes stale ones.

/// Extension of a Chrome swap file (`save.srm.crswap`).
pub const BROWSER_SWAP_FILE_EXT: &str = "crswap";

/// A swap file older than this cannot belong to a write still in progress.
#[cfg(feature = "native")]
pub const STALE_BROWSER_SWAP_AGE: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

/// Whether `file_name` is a browser swap file (`*.crswap`, any case).
pub fn is_browser_swap_file(file_name: &str) -> bool {
    file_name.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty() && ext.eq_ignore_ascii_case(BROWSER_SWAP_FILE_EXT)
    })
}

/// Deletes the swap files directly in `dir` last modified more than `max_age` ago
/// (best effort: an unreadable folder or a busy file is left alone). Returns how many
/// were removed.
#[cfg(feature = "native")]
pub fn remove_stale_browser_swap_files(
    dir: &std::path::Path,
    max_age: std::time::Duration,
) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let now = std::time::SystemTime::now();
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_str().is_some_and(is_browser_swap_file) {
            continue;
        }
        let stale = entry
            .metadata()
            .ok()
            .filter(std::fs::Metadata::is_file)
            .and_then(|meta| meta.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > max_age);
        if stale && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_swap_files_only() {
        for name in [
            "save.srm.crswap",
            "web-slot1.state.CRSWAP",
            "1700-web.json.crswap",
        ] {
            assert!(is_browser_swap_file(name), "{name}");
        }
        for name in [
            "save.srm",
            "crswap",
            ".crswap",
            "save.crswap.tmp",
            "web-auto.state",
        ] {
            assert!(!is_browser_swap_file(name), "{name}");
        }
    }

    #[cfg(feature = "native")]
    #[test]
    fn removes_only_stale_swap_files() {
        use std::time::Duration;
        let dir = tempfile::tempdir().unwrap();
        for name in ["save.srm.crswap", "save.srm", "web-slot0.state"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        // Fresh: a write may still be in progress.
        assert_eq!(
            remove_stale_browser_swap_files(dir.path(), STALE_BROWSER_SWAP_AGE),
            0
        );
        assert!(dir.path().join("save.srm.crswap").exists());
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            remove_stale_browser_swap_files(dir.path(), Duration::from_millis(5)),
            1
        );
        assert!(!dir.path().join("save.srm.crswap").exists());
        assert!(
            dir.path().join("save.srm").exists() && dir.path().join("web-slot0.state").exists()
        );
        assert_eq!(
            remove_stale_browser_swap_files(&dir.path().join("missing"), Duration::ZERO),
            0
        );
    }
}
