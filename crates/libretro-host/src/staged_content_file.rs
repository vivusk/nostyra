//! Staged content: preloaded bytes (a soft-patched ROM, or a ROM whose path is too
//! long for the core) written to `<staging dir>/content.<ext>` for a core that only
//! loads content by path (`need_fullpath`). The extension is kept because cores such
//! as Genesis Plus GX pick the system by it. Only cartridges are staged, and only at
//! load: a core that re-opened its content later would find the file gone.

use std::path::{Path, PathBuf};

use crate::host_error::{HostError, HostResult};

/// Longest staged path cores copy without truncating (256-byte path buffers).
const MAX_PATH_BYTES: usize = 255;
const FILE_STEM: &str = "content";

/// A staged content file; deleted (with its directory, if then empty) on drop.
#[derive(Debug)]
pub(crate) struct StagedContentFile {
    path: PathBuf,
}

impl StagedContentFile {
    /// Writes `bytes` to `dir/content.<ext>` (`content` without an extension),
    /// creating `dir`. A staged path of 256 UTF-8 bytes or more fails before
    /// writing: the core would truncate it, as it would the original path.
    pub(crate) fn write(dir: &Path, ext: Option<&str>, bytes: &[u8]) -> HostResult<Self> {
        let path = match ext {
            Some(ext) if !ext.is_empty() => dir.join(format!("{FILE_STEM}.{ext}")),
            _ => dir.join(FILE_STEM),
        };
        let stage_error = |source| HostError::StageContent {
            path: path.clone(),
            source,
        };
        if path.to_string_lossy().len() > MAX_PATH_BYTES {
            return Err(stage_error(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "staged path is 256 bytes or longer; use a shorter data folder",
            )));
        }
        std::fs::create_dir_all(dir).map_err(stage_error)?;
        std::fs::write(&path, bytes).map_err(stage_error)?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagedContentFile {
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_file(&self.path) {
            tracing::warn!(path = %self.path.display(), %err, "cannot delete staged content");
            return;
        }
        // Only succeeds when empty; another session's file keeps the directory.
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::remove_dir(dir);
        }
    }
}

#[cfg(test)]
#[path = "staged_content_file_tests.rs"]
mod tests;
