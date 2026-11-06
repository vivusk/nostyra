//! Owns the extracted ROM for a session. The original --rom remains the game's
//! save/config/session identity; only core and achievement I/O use `path`.

use std::path::{Path, PathBuf};

use game_library::{hash_rom_bytes, is_rom_archive, read_rom_archive};
use system_catalog::SystemSpec;

use crate::runner_exit_codes::{validate_rom, RomValidationError};

pub struct PreparedContent {
    pub path: PathBuf,
    pub system: &'static SystemSpec,
    pub content_sha1: Option<String>,
    // Outlives the core because main holds this value until loaded_session_run ends.
    _archive_dir: Option<tempfile::TempDir>,
}

impl PreparedContent {
    pub fn prepare(
        rom: &Path,
        selected_system: Option<&str>,
        staging_dir: &Path,
    ) -> Result<Self, RomValidationError> {
        if !is_rom_archive(rom) {
            return Ok(Self {
                path: rom.into(),
                system: validate_rom(rom, selected_system)?,
                content_sha1: None,
                _archive_dir: None,
            });
        }
        let failure = |reason: String| RomValidationError::Archive {
            path: rom.into(),
            reason,
        };
        let content = read_rom_archive(rom).map_err(|err| failure(err.to_string()))?;
        if selected_system.is_some_and(|id| id != content.system.id) {
            return Err(failure(format!("ROM system in ZIP is {}, which does not match the selected system; rescan the library", content.system.id)));
        }
        let content_sha1 = hash_rom_bytes(&content.bytes, content.system).sha1;
        std::fs::create_dir_all(staging_dir).map_err(|err| failure(err.to_string()))?;
        let directory = tempfile::Builder::new()
            .prefix("archive-")
            .tempdir_in(staging_dir)
            .map_err(|err| failure(err.to_string()))?;
        let ext = Path::new(&content.name)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        // A short generated filename, never a member-controlled extraction path.
        let path = directory.path().join(format!("content.{ext}"));
        std::fs::write(&path, content.bytes).map_err(|err| failure(err.to_string()))?;
        Ok(Self {
            path,
            system: content.system,
            content_sha1: Some(content_sha1),
            _archive_dir: Some(directory),
        })
    }
}
