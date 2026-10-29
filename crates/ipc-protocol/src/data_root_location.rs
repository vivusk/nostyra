//! The data folder the user chose in Settings → Data folder. One UTF-8 path in
//! `data-location.txt` under the per-user config dir (`%APPDATA%\Nostyra\config`), or
//! beside the exes in a portable copy: outside the data root, so it survives moving
//! the data root away.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::shared_data_paths::{default_data_root, portable_dir};

const LOCATION_FILE_NAME: &str = "data-location.txt";

/// `<exe dir>/data-location.txt` in a portable copy (it never touches the user's
/// AppData), else `<config dir>/data-location.txt`; `None` if the OS has no home dir.
pub fn data_location_file() -> Option<PathBuf> {
    if let Some(dir) = portable_dir() {
        return Some(dir.join(LOCATION_FILE_NAME));
    }
    directories::ProjectDirs::from("", "", "Nostyra")
        .map(|dirs| dirs.config_dir().join(LOCATION_FILE_NAME))
}

/// The data root to use: the chosen folder while it exists, else the default. A
/// chosen folder that is gone (drive unplugged, deleted) falls back to the default
/// instead of failing the start.
pub fn data_root() -> Option<PathBuf> {
    let chosen = data_location_file().and_then(|file| read_data_location(&file));
    match chosen {
        Some(root) if root.is_dir() => Some(root),
        _ => default_data_root(),
    }
}

/// The path stored in `file`; `None` when the file is missing, unreadable or blank.
pub fn read_data_location(file: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(file).ok()?;
    let path = text.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// Stores `root` in `file` (write to a temp file, then rename: never half-written).
pub fn write_data_location(file: &Path, root: &Path) -> io::Result<()> {
    let text = root.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "data folder path is not UTF-8")
    })?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = file.with_extension("txt.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_round_trips_and_blank_means_default() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config").join(LOCATION_FILE_NAME);
        assert_eq!(read_data_location(&file), None);
        let root = dir.path().join("Nostyra data");
        write_data_location(&file, &root).unwrap();
        assert_eq!(read_data_location(&file), Some(root));
        fs::write(&file, "  \n").unwrap();
        assert_eq!(read_data_location(&file), None);
    }
}
