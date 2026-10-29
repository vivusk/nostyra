//! What a session needs to start: core, content, directories, preloaded bytes.

use std::ffi::CString;
use std::path::Path;

use crate::host_error::{HostError, HostResult};
use crate::hw_render::HwRenderProvider;

/// Paths a session needs. Directories are reported to the core via the
/// `GET_SYSTEM_DIRECTORY` / `GET_SAVE_DIRECTORY` environment commands.
#[derive(Clone, Copy, Debug)]
pub struct SessionConfig<'a> {
    pub core_path: &'a Path,
    pub rom_path: &'a Path,
    pub system_dir: &'a Path,
    pub save_dir: &'a Path,
    /// Preloaded ROM bytes (e.g. soft-patched by the runner); `None` = read `rom_path`.
    /// Cores that need the full path get them through a staged file (`staging_dir`).
    pub rom_data: Option<&'a [u8]>,
    /// Where preloaded bytes are written for cores that load content by path
    /// (`need_fullpath`); the file is deleted when the session ends. `None`: such
    /// cores read `rom_path` and the preloaded bytes are ignored with a warning.
    pub staging_dir: Option<&'a Path>,
    /// Core options answered via `GET_VARIABLE` as `(key, value)`; other keys get
    /// the core's default.
    pub core_options: &'a [(String, String)],
    /// Builds graphics contexts for cores that render on the GPU (`SET_HW_RENDER`).
    /// `None`: software-only session (hardware requests are refused).
    pub hw_render: Option<&'a dyn HwRenderProvider>,
    /// Never unmap the core library, from the start of the load on (the drop still
    /// unloads the game and deinits): Dolphin hangs in `FreeLibrary` after a failed
    /// boot or any `retro_unserialize` (M14). Registry `CoreSpec::keep_library_loaded`.
    pub keep_library_loaded: bool,
}

/// `path` as a C string for the core (`InvalidPath` with an interior NUL).
pub(crate) fn c_path(path: &Path) -> HostResult<CString> {
    CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| HostError::InvalidPath(path.to_path_buf()))
}

/// Options as C strings; a pair with an interior NUL is dropped with a warning.
pub(crate) fn core_option_cstrings(options: &[(String, String)]) -> Vec<(CString, CString)> {
    options
        .iter()
        .filter_map(|(key, value)| {
            match (CString::new(key.as_str()), CString::new(value.as_str())) {
                (Ok(key), Ok(value)) => Some((key, value)),
                _ => {
                    tracing::warn!(key, "core option with a NUL byte ignored");
                    None
                }
            }
        })
        .collect()
}
