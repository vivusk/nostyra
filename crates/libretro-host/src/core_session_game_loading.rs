//! Game loading: ROM bytes from disk or preloaded by the caller (soft-patching).
//! A core that loads by path gets preloaded bytes as a staged file.

use std::path::Path;

use crate::core_session::CoreSession;
use crate::environment_core_messages::last_core_error;
use crate::ffi_types::RetroGameInfo;
use crate::host_error::{HostError, HostResult};
use crate::session_config::c_path;
use crate::staged_content_file::StagedContentFile;

impl CoreSession {
    /// Reads (or takes the preloaded) ROM and calls `retro_load_game`. A rejection
    /// carries the core's reason: melonDS DS sends bad-ROM texts as messages ("ROM
    /// isn't valid, …") and config errors as error log lines only.
    pub(crate) fn load_game(
        &mut self,
        rom_path: &Path,
        preloaded: Option<&[u8]>,
        staging_dir: Option<&Path>,
    ) -> HostResult<()> {
        let (need_fullpath, core_info) = self.core.system_info();
        self.core_info = core_info;
        match (need_fullpath, preloaded, staging_dir) {
            (true, Some(bytes), Some(dir)) => {
                let ext = rom_path.extension().and_then(|ext| ext.to_str());
                let staged = StagedContentFile::write(dir, ext, bytes)?;
                self.rom_path = c_path(staged.path())?;
                tracing::info!(path = %staged.path().display(), "core loads by path; preloaded content staged");
                self.staged = Some(staged);
            }
            (true, Some(_), None) => {
                tracing::warn!("core loads ROMs by path; preloaded (patched) ROM data ignored");
            }
            _ => {}
        }
        if !need_fullpath {
            self.rom_data = match preloaded {
                Some(bytes) => bytes.to_vec(),
                None => std::fs::read(rom_path).map_err(|source| HostError::ReadRom {
                    path: rom_path.to_path_buf(),
                    source,
                })?,
            };
        }
        let game = RetroGameInfo {
            path: self.rom_path.as_ptr(),
            data: if need_fullpath {
                std::ptr::null()
            } else {
                self.rom_data.as_ptr().cast()
            },
            size: self.rom_data.len(),
            meta: std::ptr::null(),
        };
        // SAFETY: `game` points at buffers owned by `self` for the session lifetime.
        if !unsafe { (self.core.load_game)(&game) } {
            let reason = self.ctx().core_messages.last().cloned();
            return Err(HostError::LoadGameRejected(reason.or_else(last_core_error)));
        }
        self.game_loaded = true;
        Ok(())
    }
}

#[cfg(test)]
#[path = "core_session_load_rejection_tests.rs"]
mod load_rejection_tests;
