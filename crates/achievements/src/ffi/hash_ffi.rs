//! rcheevos v12.5.0 `rc_hash` declarations (`include/rc_hash.h`). Layout matches the
//! build defines: `RC_HASH_NO_ENCRYPTED` (no `encryption` member), disc support on
//! (`cdreader` present). The Rust file / CD readers behind these callbacks are phase 02.

use std::ffi::{c_char, c_int, c_void};

pub type MessageFn = unsafe extern "C" fn(message: *const c_char, iterator: *const RcHashIterator);

pub type FileOpenFn = unsafe extern "C" fn(path_utf8: *const c_char) -> *mut c_void;
pub type FileSeekFn = unsafe extern "C" fn(handle: *mut c_void, offset: i64, origin: c_int);
pub type FileTellFn = unsafe extern "C" fn(handle: *mut c_void) -> i64;
pub type FileReadFn =
    unsafe extern "C" fn(handle: *mut c_void, buffer: *mut c_void, requested: usize) -> usize;
pub type FileCloseFn = unsafe extern "C" fn(handle: *mut c_void);

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RcHashFilereader {
    pub open: Option<FileOpenFn>,
    pub seek: Option<FileSeekFn>,
    pub tell: Option<FileTellFn>,
    pub read: Option<FileReadFn>,
    pub close: Option<FileCloseFn>,
}

pub type CdOpenTrackFn = unsafe extern "C" fn(path: *const c_char, track: u32) -> *mut c_void;
pub type CdOpenTrackIteratorFn = unsafe extern "C" fn(
    path: *const c_char,
    track: u32,
    iterator: *const RcHashIterator,
) -> *mut c_void;
pub type CdReadSectorFn = unsafe extern "C" fn(
    track: *mut c_void,
    sector: u32,
    buffer: *mut c_void,
    requested: usize,
) -> usize;
pub type CdCloseTrackFn = unsafe extern "C" fn(track: *mut c_void);
pub type CdFirstTrackSectorFn = unsafe extern "C" fn(track: *mut c_void) -> u32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RcHashCdreader {
    /// Legacy; `rc_hash_merge_callbacks` (rc_client path) copies the cdreader only
    /// when this one is set, so set it too.
    pub open_track: Option<CdOpenTrackFn>,
    pub read_sector: Option<CdReadSectorFn>,
    pub close_track: Option<CdCloseTrackFn>,
    pub first_track_sector: Option<CdFirstTrackSectorFn>,
    /// Preferred by `rc_cd_open_track` when set.
    pub open_track_iterator: Option<CdOpenTrackIteratorFn>,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RcHashCallbacks {
    pub verbose_message: Option<MessageFn>,
    pub error_message: Option<MessageFn>,
    pub filereader: RcHashFilereader,
    pub cdreader: RcHashCdreader,
}

#[repr(C)]
pub struct RcHashIterator {
    pub buffer: *const u8,
    pub buffer_size: usize,
    pub consoles: [u8; 12],
    pub index: c_int,
    pub path: *const c_char,
    pub userdata: *mut c_void,
    pub callbacks: RcHashCallbacks,
}

unsafe extern "C" {
    /// Resets callbacks to the defaults (std filereader, default cdreader), strdup's path.
    pub fn rc_hash_initialize_iterator(
        iterator: *mut RcHashIterator,
        path: *const c_char,
        buffer: *const u8,
        buffer_size: usize,
    );
    pub fn rc_hash_destroy_iterator(iterator: *mut RcHashIterator);
    /// Non-zero on success; `hash` gets 32 hex chars + NUL.
    pub fn rc_hash_generate(
        hash: *mut c_char,
        console_id: u32,
        iterator: *const RcHashIterator,
    ) -> c_int;
    /// Extension-guessing variant (console unknown).
    pub fn rc_hash_iterate(hash: *mut c_char, iterator: *mut RcHashIterator) -> c_int;
}
