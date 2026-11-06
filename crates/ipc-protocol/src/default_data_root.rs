//! Desktop-only (feature `native`): the data root used when the user never chose one
//! in Settings → Data folder (`data-location.txt` wins over this).
//!
//! New installs keep their data in `Documents\Nostyra` (`~/Documents/Nostyra` on
//! macOS / Linux) so the web edition can share the folder: Chrome refuses to open
//! folders inside AppData. Installs from before that keep the per-user app-data
//! folder (`%APPDATA%\Nostyra\data`) while its database is there — nothing is moved
//! silently. Portable copies always use `<exe dir>/data`.

use std::path::{Path, PathBuf};

use crate::native_data_roots::portable_dir;
use crate::shared_data_paths::{DATABASE_FILE_NAME, PORTABLE_DATA_DIR};

/// Folder created under the user's Documents folder for a new install.
pub const DOCUMENTS_DATA_DIR: &str = "Nostyra";

/// Default data root: `<exe dir>/data` in a portable copy, else the legacy per-user
/// app-data folder when it holds a database (existing install), else
/// `<Documents>/Nostyra`, else (no Documents folder) the legacy folder. Runner and
/// launcher both call this, so their defaults cannot drift; `None` if the OS has no
/// home directory.
pub fn default_data_root() -> Option<PathBuf> {
    let legacy =
        directories::ProjectDirs::from("", "", "Nostyra").map(|dirs| dirs.data_dir().to_path_buf());
    let documents =
        directories::UserDirs::new().and_then(|dirs| dirs.document_dir().map(Path::to_path_buf));
    choose_default_data_root(portable_dir().as_deref(), legacy, documents)
}

/// [`default_data_root`]'s rule over given candidates (pure apart from checking for
/// the legacy database file; for tests). `portable` = the portable copy's exe folder.
pub fn choose_default_data_root(
    portable: Option<&Path>,
    legacy: Option<PathBuf>,
    documents: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(dir) = portable {
        return Some(dir.join(PORTABLE_DATA_DIR));
    }
    if let Some(legacy) = legacy.as_ref() {
        if legacy.join(DATABASE_FILE_NAME).is_file() {
            return Some(legacy.clone());
        }
    }
    documents.map(|dir| dir.join(DOCUMENTS_DATA_DIR)).or(legacy)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dirs {
        _temp: tempfile::TempDir,
        exe: PathBuf,
        legacy: PathBuf,
        documents: PathBuf,
    }

    fn dirs() -> Dirs {
        let temp = tempfile::tempdir().unwrap();
        let path = |name: &str| temp.path().join(name);
        Dirs {
            exe: path("exe"),
            legacy: path("AppData/Roaming/Nostyra/data"),
            documents: path("Documents"),
            _temp: temp,
        }
    }

    #[test]
    fn fresh_install_uses_documents() {
        let d = dirs();
        let root = choose_default_data_root(None, Some(d.legacy), Some(d.documents.clone()));
        assert_eq!(root, Some(d.documents.join("Nostyra")));
    }

    #[test]
    fn existing_install_keeps_the_legacy_folder() {
        let d = dirs();
        std::fs::create_dir_all(&d.legacy).unwrap();
        // Logs or an empty folder alone are not an install: only the database counts.
        std::fs::create_dir_all(d.legacy.join("logs")).unwrap();
        let fresh =
            choose_default_data_root(None, Some(d.legacy.clone()), Some(d.documents.clone()));
        assert_eq!(fresh, Some(d.documents.join("Nostyra")));
        std::fs::write(d.legacy.join(DATABASE_FILE_NAME), b"db").unwrap();
        let root = choose_default_data_root(None, Some(d.legacy.clone()), Some(d.documents));
        assert_eq!(root, Some(d.legacy));
    }

    #[test]
    fn portable_copy_uses_its_own_folder() {
        let d = dirs();
        std::fs::create_dir_all(&d.legacy).unwrap();
        std::fs::write(d.legacy.join(DATABASE_FILE_NAME), b"db").unwrap();
        let root = choose_default_data_root(Some(&d.exe), Some(d.legacy), Some(d.documents));
        assert_eq!(root, Some(d.exe.join("data")));
    }

    #[test]
    fn no_documents_folder_falls_back_to_the_legacy_folder() {
        let d = dirs();
        let root = choose_default_data_root(None, Some(d.legacy.clone()), None);
        assert_eq!(root, Some(d.legacy));
        assert_eq!(choose_default_data_root(None, None, None), None);
    }

    #[test]
    fn os_default_ends_in_a_nostyra_folder() {
        let root = default_data_root().unwrap();
        // Linux XDG paths use the lower-case app name (`~/.local/share/nostyra`).
        assert!(
            root.to_string_lossy().to_lowercase().contains("nostyra"),
            "{}",
            root.display()
        );
    }
}
