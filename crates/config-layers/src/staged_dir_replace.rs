//! Replacing a whole directory with a fully prepared one beside it: the core
//! system files a core needs (`<cores>/PPSSPP/` from the core download, then
//! `<data>/system/PPSSPP/` from the runner). Not atomic: `rename` cannot overwrite
//! a non-empty directory, so the old one moves aside first. A crash in between
//! leaves `<dest>.old` (and maybe no `dest`); the next replace cleans it up, and a
//! missing `dest` simply reads as "not installed".

use std::io;
use std::path::{Path, PathBuf};

/// Sibling working names of `dest`: (`<dest>.tmp` to prepare the new copy in,
/// `<dest>.old` for the previous one during the swap).
pub fn staging_dirs(dest: &Path) -> (PathBuf, PathBuf) {
    let sibling = |suffix: &str| {
        let mut name = dest.file_name().unwrap_or_default().to_os_string();
        name.push(suffix);
        dest.with_file_name(name)
    };
    (sibling(".tmp"), sibling(".old"))
}

/// Removes leftover `<dest>.tmp` / `<dest>.old` of an interrupted replace.
pub fn remove_stale_staging(dest: &Path) -> io::Result<()> {
    let (tmp, old) = staging_dirs(dest);
    for dir in [tmp, old] {
        match std::fs::remove_dir_all(&dir) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => {
                return Err(named(err, "cannot remove", &dir));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Swaps `staged` (complete, same volume) in as `dest`: an existing `dest` →
/// `<dest>.old`, `staged` → `dest`, then `.old` deleted (best effort). If the
/// second rename fails the old copy is put back.
pub fn replace_dir(staged: &Path, dest: &Path) -> io::Result<()> {
    let (_, old) = staging_dirs(dest);
    let _ = std::fs::remove_dir_all(&old); // stale from an interrupted swap
    let had_dest = dest.exists();
    if had_dest {
        std::fs::rename(dest, &old).map_err(|err| named(err, "cannot move aside", dest))?;
    }
    if let Err(err) = std::fs::rename(staged, dest) {
        if had_dest {
            let _ = std::fs::rename(&old, dest);
        }
        return Err(named(err, "cannot install", dest));
    }
    if had_dest {
        if let Err(err) = std::fs::remove_dir_all(&old) {
            tracing::warn!(dir = %old.display(), %err, "old copy left behind");
        }
    }
    Ok(())
}

fn named(err: io::Error, action: &str, path: &Path) -> io::Error {
    io::Error::new(err.kind(), format!("{action} {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(root: &Path, name: &str, file: &str, text: &str) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), text).unwrap();
        dir
    }

    #[test]
    fn replaces_an_existing_directory_and_leaves_no_siblings() {
        let root = tempfile::tempdir().unwrap();
        let dest = dir_with(root.path(), "PPSSPP", "compat.ini", "old");
        let staged = dir_with(root.path(), "PPSSPP.tmp", "compat.ini", "new");
        replace_dir(&staged, &dest).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("compat.ini")).unwrap(),
            "new"
        );
        let (tmp, old) = staging_dirs(&dest);
        assert!(!tmp.exists() && !old.exists());
    }

    #[test]
    fn installs_where_nothing_was_and_cleans_stale_leftovers() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("PPSSPP");
        dir_with(root.path(), "PPSSPP.old", "x", "stale");
        let staged = dir_with(root.path(), "PPSSPP.tmp", "compat.ini", "new");
        replace_dir(&staged, &dest).unwrap();
        assert!(dest.join("compat.ini").is_file());
        dir_with(root.path(), "PPSSPP.tmp", "y", "stale");
        remove_stale_staging(&dest).unwrap();
        let (tmp, old) = staging_dirs(&dest);
        assert!(!tmp.exists() && !old.exists() && dest.exists());
    }

    #[test]
    fn failed_install_puts_the_old_copy_back() {
        let root = tempfile::tempdir().unwrap();
        let dest = dir_with(root.path(), "PPSSPP", "compat.ini", "old");
        let missing = root.path().join("nothing-staged");
        assert!(replace_dir(&missing, &dest).is_err());
        assert_eq!(
            std::fs::read_to_string(dest.join("compat.ini")).unwrap(),
            "old"
        );
    }
}
