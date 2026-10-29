//! Opens a core shared library and resolves every `retro_*` entry point up front,
//! so a broken/non-libretro library fails at load time instead of mid-session.

use std::ffi::{c_char, c_uint, c_void, CStr};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::ffi_types::*;
use crate::host_error::{HostError, HostResult};

/// Resolved entry points. Fn pointers stay valid while `_lib` is alive, which is
/// guaranteed because they live in the same struct and are never handed out.
pub(crate) struct CoreLibrary {
    pub api_version: unsafe extern "C" fn() -> c_uint,
    pub get_system_info: unsafe extern "C" fn(*mut RetroSystemInfo),
    pub get_system_av_info: unsafe extern "C" fn(*mut RetroSystemAvInfo),
    pub set_environment: unsafe extern "C" fn(RetroEnvironmentFn),
    pub set_video_refresh: unsafe extern "C" fn(RetroVideoRefreshFn),
    pub set_audio_sample: unsafe extern "C" fn(RetroAudioSampleFn),
    pub set_audio_sample_batch: unsafe extern "C" fn(RetroAudioSampleBatchFn),
    pub set_input_poll: unsafe extern "C" fn(RetroInputPollFn),
    pub set_input_state: unsafe extern "C" fn(RetroInputStateFn),
    pub init: unsafe extern "C" fn(),
    pub deinit: unsafe extern "C" fn(),
    pub reset: unsafe extern "C" fn(),
    pub run: unsafe extern "C" fn(),
    pub serialize_size: unsafe extern "C" fn() -> usize,
    pub serialize: unsafe extern "C" fn(*mut c_void, usize) -> bool,
    pub unserialize: unsafe extern "C" fn(*const c_void, usize) -> bool,
    pub load_game: unsafe extern "C" fn(*const RetroGameInfo) -> bool,
    pub unload_game: unsafe extern "C" fn(),
    pub get_memory_data: unsafe extern "C" fn(c_uint) -> *mut c_void,
    pub get_memory_size: unsafe extern "C" fn(c_uint) -> usize,
    /// `retro_set_controller_port_device(port, device)`. A mandatory libretro export,
    /// resolved as optional only for tolerance; called solely for cores whose spec
    /// lists port devices (see `core_session_port_devices`).
    pub set_controller_port_device: Option<unsafe extern "C" fn(c_uint, c_uint)>,
    /// `None` once [`Self::keep_loaded`] leaked it.
    pub(crate) _lib: Option<libloading::Library>,
}

/// Core identity from `retro_get_system_info` (used e.g. to validate save states).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoreInfo {
    pub name: String,
    pub version: String,
}

/// Copies a core-owned C string (may be null).
fn owned_c_str(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: libretro guarantees NUL-terminated static strings in system info.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// Resolves `name` as a fn pointer of type `T`.
///
/// # Safety
/// `T` must be the exact C signature of the exported symbol.
unsafe fn symbol<T: Copy>(lib: &libloading::Library, name: &'static str) -> HostResult<T> {
    // SAFETY: caller guarantees the signature; name is NUL-free ASCII.
    unsafe { lib.get::<T>(name.as_bytes()) }
        .map(|sym| *sym)
        .map_err(|_| HostError::MissingSymbol(name))
}

impl CoreLibrary {
    /// Loads the library, resolves all symbols and checks `retro_api_version() == 1`.
    pub fn open(path: &Path) -> HostResult<Self> {
        log_core_identity(path);
        // SAFETY: loading a core runs its static initialisers. Cores are trusted
        // native code by design (user-selected or from Nostyra's cores dir).
        let lib =
            unsafe { libloading::Library::new(path) }.map_err(|source| HostError::LoadLibrary {
                path: path.to_path_buf(),
                source,
            })?;
        // SAFETY: every signature below mirrors libretro.h for API version 1.
        let core = unsafe {
            Self {
                api_version: symbol(&lib, "retro_api_version")?,
                get_system_info: symbol(&lib, "retro_get_system_info")?,
                get_system_av_info: symbol(&lib, "retro_get_system_av_info")?,
                set_environment: symbol(&lib, "retro_set_environment")?,
                set_video_refresh: symbol(&lib, "retro_set_video_refresh")?,
                set_audio_sample: symbol(&lib, "retro_set_audio_sample")?,
                set_audio_sample_batch: symbol(&lib, "retro_set_audio_sample_batch")?,
                set_input_poll: symbol(&lib, "retro_set_input_poll")?,
                set_input_state: symbol(&lib, "retro_set_input_state")?,
                init: symbol(&lib, "retro_init")?,
                deinit: symbol(&lib, "retro_deinit")?,
                reset: symbol(&lib, "retro_reset")?,
                run: symbol(&lib, "retro_run")?,
                serialize_size: symbol(&lib, "retro_serialize_size")?,
                serialize: symbol(&lib, "retro_serialize")?,
                unserialize: symbol(&lib, "retro_unserialize")?,
                load_game: symbol(&lib, "retro_load_game")?,
                unload_game: symbol(&lib, "retro_unload_game")?,
                get_memory_data: symbol(&lib, "retro_get_memory_data")?,
                get_memory_size: symbol(&lib, "retro_get_memory_size")?,
                set_controller_port_device: symbol(&lib, "retro_set_controller_port_device").ok(),
                _lib: Some(lib),
            }
        };
        // SAFETY: symbol resolved above; retro_api_version has no preconditions.
        let version = unsafe { (core.api_version)() };
        if version != RETRO_API_VERSION {
            return Err(HostError::ApiVersion(version));
        }
        Ok(core)
    }
}

#[cfg(test)]
impl CoreLibrary {
    /// A handle on this process, to back fake entry points built by tests.
    pub(crate) fn this_process() -> libloading::Library {
        #[cfg(windows)]
        let os = libloading::os::windows::Library::this().expect("own module");
        #[cfg(unix)]
        let os = libloading::os::unix::Library::this();
        os.into()
    }
}

impl CoreLibrary {
    /// Never unloads the library (it stays mapped until the process exits): for a core
    /// left mid-load whose own threads may still run its code (see `CoreSession` drop).
    pub(crate) fn keep_loaded(&mut self) {
        std::mem::forget(self._lib.take());
    }

    /// [`Self::keep_loaded`] when `keep` (`SessionConfig::keep_library_loaded`).
    pub(crate) fn keep_loaded_if(&mut self, keep: bool) {
        if keep {
            self.keep_loaded();
        }
    }

    /// `retro_get_system_info`: (need_fullpath, identity).
    pub fn system_info(&self) -> (bool, CoreInfo) {
        // SAFETY: zeroed POD out-param filled by the core; strings are core-owned statics.
        let mut info: RetroSystemInfo = unsafe { std::mem::zeroed() };
        // SAFETY: valid out-param; may be called before retro_init per libretro.h.
        unsafe { (self.get_system_info)(&mut info) };
        let identity = CoreInfo {
            name: owned_c_str(info.library_name),
            version: owned_c_str(info.library_version),
        };
        (info.need_fullpath, identity)
    }
}

/// Logs path + SHA256 so a misbehaving core can be identified from logs.
/// Best effort: a read failure here is reported by `Library::new` anyway.
fn log_core_identity(path: &Path) {
    match std::fs::read(path) {
        Ok(bytes) => {
            let hex: String = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            tracing::info!(core = %path.display(), sha256 = %hex, "loading libretro core");
        }
        Err(err) => tracing::warn!(core = %path.display(), %err, "cannot hash core"),
    }
}
