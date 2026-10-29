//! rc_hash filereader over `std::fs::File`: rcheevos hands UTF-8 paths, so Unicode
//! file names open on every OS (the C default goes through `fopen`).

use std::ffi::{c_char, c_int, c_void, CStr};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use super::hash_ffi::RcHashFilereader;

pub fn file_reader() -> RcHashFilereader {
    RcHashFilereader {
        open: Some(file_open),
        seek: Some(file_seek),
        tell: Some(file_tell),
        read: Some(file_read),
        close: Some(file_close),
    }
}

/// # Safety
/// `ptr` is NULL or a NUL-terminated string valid for `'a`.
pub unsafe fn path_of<'a>(ptr: *const c_char) -> Option<&'a Path> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: caller contract.
    unsafe { CStr::from_ptr(ptr) }.to_str().ok().map(Path::new)
}

unsafe extern "C" fn file_open(path: *const c_char) -> *mut c_void {
    // SAFETY: rcheevos passes a NUL-terminated UTF-8 path valid for the call.
    match unsafe { path_of(path) }.map(File::open) {
        Some(Ok(file)) => Box::into_raw(Box::new(file)).cast(),
        _ => std::ptr::null_mut(),
    }
}

/// # Safety
/// `handle` came from `file_open` and is not closed yet.
unsafe fn file<'a>(handle: *mut c_void) -> Option<&'a mut File> {
    // SAFETY: caller contract (NULL is tolerated).
    unsafe { handle.cast::<File>().as_mut() }
}

unsafe extern "C" fn file_seek(handle: *mut c_void, offset: i64, origin: c_int) {
    let pos = match origin {
        0 => SeekFrom::Start(offset.max(0) as u64),
        1 => SeekFrom::Current(offset),
        _ => SeekFrom::End(offset),
    };
    // SAFETY: live handle from `file_open`.
    if let Some(f) = unsafe { file(handle) } {
        let _ = f.seek(pos); // a failed seek shows up as a short read
    }
}

unsafe extern "C" fn file_tell(handle: *mut c_void) -> i64 {
    // SAFETY: live handle from `file_open`.
    unsafe { file(handle) }
        .and_then(|f| f.stream_position().ok())
        .and_then(|p| i64::try_from(p).ok())
        .unwrap_or(-1)
}

unsafe extern "C" fn file_read(
    handle: *mut c_void,
    buffer: *mut c_void,
    requested: usize,
) -> usize {
    if buffer.is_null() || requested == 0 {
        return 0;
    }
    // SAFETY: live handle from `file_open`.
    let Some(f) = (unsafe { file(handle) }) else {
        return 0;
    };
    // SAFETY: rcheevos hands a writable buffer of `requested` bytes.
    let out = unsafe { std::slice::from_raw_parts_mut(buffer.cast::<u8>(), requested) };
    let mut done = 0;
    while done < requested {
        match f.read(&mut out[done..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => done += n,
        }
    }
    done
}

unsafe extern "C" fn file_close(handle: *mut c_void) {
    if !handle.is_null() {
        // SAFETY: handle from `file_open`, closed once.
        drop(unsafe { Box::from_raw(handle.cast::<File>()) });
    }
}
