//! `GET_SAVESTATE_CONTEXT`: tells the core why the frontend is saving/loading a
//! state, so it can take faster paths for RAM-only states (run-ahead, rewind).

use std::ffi::c_int;

/// Purpose of a serialize/unserialize. Answered to the core only while that call
/// is in progress; `Normal` otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SavestateContext {
    /// Save-state slots: the state may be written to disk and loaded by another build.
    #[default]
    Normal,
    /// Run-ahead/rewind on this instance: RAM only, same binary, same session.
    RunaheadSameInstance,
}

impl SavestateContext {
    /// `retro_savestate_context` value.
    pub(crate) fn as_raw(self) -> c_int {
        match self {
            Self::Normal => 0,
            Self::RunaheadSameInstance => 1,
        }
    }
}
