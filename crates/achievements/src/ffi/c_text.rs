//! Copies C strings out of rcheevos memory into owned Rust strings, and Rust strings
//! into NUL-terminated buffers for calls (interior NULs never reach C).

use std::ffi::{c_char, CStr, CString};

/// Owned copy of a NUL-terminated C string; NULL → empty. Invalid UTF-8 is replaced.
///
/// # Safety
/// `ptr` is NULL or points to a NUL-terminated string valid for this call.
pub unsafe fn owned_text(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: caller guarantees a valid NUL-terminated string.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// Owned copy of a fixed-size `char[N]` field (stops at the first NUL or at N).
pub fn fixed_text(chars: &[c_char]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// NUL-terminated copy for a C call; interior NULs are dropped (never valid in the
/// identifiers, hashes and paths rcheevos receives).
pub fn c_string(text: &str) -> CString {
    CString::new(text.replace('\0', "")).unwrap_or_default()
}

/// Overwrites a CString's bytes before freeing it (password handling).
pub fn wipe_c_string(text: CString) {
    let mut bytes = text.into_bytes_with_nul();
    bytes.iter_mut().for_each(|byte| *byte = 0);
    std::hint::black_box(&bytes);
}
