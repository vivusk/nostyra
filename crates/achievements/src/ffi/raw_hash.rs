//! One-call `rc_hash_generate` with our readers (disc swap hashing, M15 D8: rc_client's
//! own change-media path ignores per-client readers), and the callback set installed
//! on every rc_client.

use std::ffi::{c_char, CStr, CString};
use std::mem::MaybeUninit;

use super::hash_cd_reader::cd_reader;
use super::hash_ffi::*;
use super::hash_file_reader::file_reader;

/// Which file / CD readers a hash uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashReaders {
    /// Rust filereader + `disc-image` cdreader.
    Ours,
    /// rcheevos' built-ins (stdio filereader; cue / gdi / bin cdreader): parity tests.
    #[cfg(test)]
    Default,
}

/// Our readers without message hooks: rc_client then routes rc_hash messages to its
/// own log (the `rcheevos` tracing target).
pub fn client_hash_callbacks() -> RcHashCallbacks {
    RcHashCallbacks {
        verbose_message: None,
        error_message: None,
        filereader: file_reader(),
        cdreader: cd_reader(),
    }
}

unsafe extern "C" fn on_verbose(message: *const c_char, _: *const RcHashIterator) {
    // SAFETY: rcheevos passes a NUL-terminated message valid for the call.
    let text = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    tracing::debug!(target: "rcheevos", "rc_hash: {text}");
}

unsafe extern "C" fn on_error(message: *const c_char, _: *const RcHashIterator) {
    // SAFETY: rcheevos passes a NUL-terminated message valid for the call.
    let text = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    tracing::warn!(target: "rcheevos", "rc_hash: {text}");
}

/// `rc_hash_generate(console_id)` of `path`, or of `buffer` (the bytes a core got)
/// when given. `None`: rcheevos produced no hash (unsupported format, unreadable).
pub fn generate_hash(
    console_id: u32,
    path: &str,
    buffer: Option<&[u8]>,
    readers: HashReaders,
) -> Option<String> {
    let c_path = CString::new(path).ok()?;
    let (data, len) = buffer.map_or((std::ptr::null(), 0), |b| (b.as_ptr(), b.len()));
    let mut iterator = MaybeUninit::<RcHashIterator>::zeroed();
    let mut hash = [0 as c_char; 33];
    // SAFETY: rcheevos initialises the iterator before it is read (all-zero is a valid
    // bit pattern for its pointers / options anyway) and it is destroyed exactly once;
    // `c_path` and `buffer` outlive both calls; `hash` has the 33 bytes rc_hash writes.
    unsafe {
        rc_hash_initialize_iterator(iterator.as_mut_ptr(), c_path.as_ptr(), data, len);
        let it = iterator.assume_init_mut();
        // Initialising resets the callbacks to the defaults: override afterwards.
        match readers {
            HashReaders::Ours => {
                it.callbacks.filereader = file_reader();
                it.callbacks.cdreader = cd_reader();
            }
            #[cfg(test)]
            HashReaders::Default => {}
        }
        it.callbacks.verbose_message = Some(on_verbose);
        it.callbacks.error_message = Some(on_error);
        let ok = rc_hash_generate(hash.as_mut_ptr(), console_id, it) != 0;
        rc_hash_destroy_iterator(it);
        ok.then(|| CStr::from_ptr(hash.as_ptr()).to_string_lossy().into_owned())
    }
}
