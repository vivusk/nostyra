//! Crash-safe file writes: write a sibling temp file, flush to disk, then rename
//! over the target, so a crash never leaves a half-written file. Shared by config
//! layers and the runner's saves (SRAM, state slots, pending sessions).

use std::io::{self, Write};
use std::path::Path;

/// Replaces `path` with `bytes` atomically; on failure no `.tmp` file is left.
/// Errors name the file, so callers can log them as-is.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let Some(file_name) = path.file_name() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} has no file name", path.display()),
        ));
    };
    let mut tmp_name = file_name.to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    let result = write_then_rename(&tmp, path, bytes);
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp); // best effort: no stray temp file
    }
    result.map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("cannot write {}: {err}", path.display()),
        )
    })
}

fn write_then_rename(tmp: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::File::create(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    // std::fs::rename replaces an existing target on Windows and Unix.
    std::fs::rename(tmp, path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // test code: panics are the failure signal
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_file_and_leaves_no_temp() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-atomic-")
            .tempdir()
            .unwrap();
        let dir = temp.path();
        let target = dir.join("game.srm");
        write_atomically(&target, b"first").unwrap();
        write_atomically(&target, b"second").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second");
        assert!(!dir.join("game.srm.tmp").exists());
    }

    #[test]
    fn failed_rename_cleans_up_temp() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-atomic-fail-")
            .tempdir()
            .unwrap();
        let dir = temp.path();
        std::fs::create_dir_all(dir.join("target")).unwrap(); // target is a directory
        let err = write_atomically(&dir.join("target"), b"x").unwrap_err();
        assert!(err.to_string().contains("target"), "{err}");
        assert!(!dir.join("target.tmp").exists());
    }
}
