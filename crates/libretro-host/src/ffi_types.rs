//! Hand-written subset of `third-party/libretro/libretro.h` (API version 1).
//!
//! Only symbols the host uses are mirrored. Hand-written instead of bindgen so
//! the layout is identical on MSVC/GCC/Clang without a libclang dependency;
//! `tests::struct_layouts_match_c` guards against drift.

use std::ffi::{c_char, c_int, c_uint, c_void};

pub const RETRO_API_VERSION: c_uint = 1;
pub const RETRO_DEVICE_NONE: c_uint = 0;
pub const RETRO_DEVICE_JOYPAD: c_uint = 1;
pub const RETRO_DEVICE_ANALOG: c_uint = 5;
/// Absolute pointer (mouse / touch): X and Y span the whole output frame,
/// -0x7FFF..0x7FFF; index = touch number.
pub const RETRO_DEVICE_POINTER: c_uint = 6;
pub const RETRO_DEVICE_ID_POINTER_X: c_uint = 0;
pub const RETRO_DEVICE_ID_POINTER_Y: c_uint = 1;
pub const RETRO_DEVICE_ID_POINTER_PRESSED: c_uint = 2;
pub const RETRO_DEVICE_ID_POINTER_COUNT: c_uint = 3;
pub const RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN: c_uint = 15;
pub const RETRO_MEMORY_SAVE_RAM: c_uint = 0;
pub const RETRO_MEMORY_RTC: c_uint = 1;
pub const RETRO_MEMORY_SYSTEM_RAM: c_uint = 2;

/// Flag OR-ed into experimental environment commands; masked off before matching.
pub const RETRO_ENVIRONMENT_EXPERIMENTAL: c_uint = 0x10000;
pub const RETRO_ENVIRONMENT_GET_CAN_DUPE: c_uint = 3;
pub const RETRO_ENVIRONMENT_SHUTDOWN: c_uint = 7;
pub const RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL: c_uint = 8;
pub const RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY: c_uint = 9;
pub const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: c_uint = 10;
pub const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: c_uint = 11;
pub const RETRO_ENVIRONMENT_GET_VARIABLE: c_uint = 15;
pub const RETRO_ENVIRONMENT_SET_VARIABLES: c_uint = 16;
pub const RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE: c_uint = 17;
pub const RETRO_ENVIRONMENT_GET_LOG_INTERFACE: c_uint = 27;
pub const RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY: c_uint = 31;
pub const RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO: c_uint = 32;
pub const RETRO_ENVIRONMENT_SET_GEOMETRY: c_uint = 37;
pub const RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION: c_uint = 52;
/// Experimental (OR'd with `RETRO_ENVIRONMENT_EXPERIMENTAL` by cores; stripped on entry).
pub const RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE: c_uint = 47;
/// `retro_av_enable_flags`: render video / audio this frame.
pub const RETRO_AV_ENABLE_VIDEO: c_int = 1;
pub const RETRO_AV_ENABLE_AUDIO: c_int = 2;
/// States taken now stay in RAM and are loaded by this binary (deprecated twin of
/// `GET_SAVESTATE_CONTEXT`, still read by older cores).
pub const RETRO_AV_ENABLE_FAST_SAVESTATES: c_int = 4;
/// `data` = `uint64_t *` (in: core quirks, out: the ones the frontend supports).
pub const RETRO_ENVIRONMENT_SET_SERIALIZATION_QUIRKS: c_uint = 44;
/// Legacy alias of `SET_SERIALIZATION_QUIRKS`, same payload.
pub const RETRO_ENVIRONMENT_SET_SERIALIZATION_QUIRKS_2: c_uint = 87;
/// Experimental; `data` = `retro_savestate_context *` (may be NULL = availability probe).
pub const RETRO_ENVIRONMENT_GET_SAVESTATE_CONTEXT: c_uint = 72;

pub const RETRO_PIXEL_FORMAT_0RGB1555: c_uint = 0;
pub const RETRO_PIXEL_FORMAT_XRGB8888: c_uint = 1;
pub const RETRO_PIXEL_FORMAT_RGB565: c_uint = 2;

pub const RETRO_LOG_DEBUG: c_uint = 0;
pub const RETRO_LOG_INFO: c_uint = 1;
pub const RETRO_LOG_WARN: c_uint = 2;

#[repr(C)]
pub struct RetroSystemInfo {
    pub library_name: *const c_char,
    pub library_version: *const c_char,
    pub valid_extensions: *const c_char,
    pub need_fullpath: bool,
    pub block_extract: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RetroGameGeometry {
    pub base_width: c_uint,
    pub base_height: c_uint,
    pub max_width: c_uint,
    pub max_height: c_uint,
    pub aspect_ratio: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RetroSystemTiming {
    pub fps: f64,
    pub sample_rate: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RetroSystemAvInfo {
    pub geometry: RetroGameGeometry,
    pub timing: RetroSystemTiming,
}

#[repr(C)]
pub struct RetroGameInfo {
    pub path: *const c_char,
    pub data: *const c_void,
    pub size: usize,
    pub meta: *const c_char,
}

/// C type is variadic `void (*)(enum retro_log_level, const char *fmt, ...)`;
/// we only ever store the C shim's address here, never call it from Rust.
pub type RetroLogPrintf = unsafe extern "C" fn(level: c_uint, fmt: *const c_char, ...);

#[repr(C)]
pub struct RetroLogCallback {
    pub log: RetroLogPrintf,
}

/// `struct retro_memory_descriptor` (`SET_MEMORY_MAPS`). `ptr` points into core
/// memory for as long as the game is loaded; `addrspace` (often NULL) is owned by the
/// host's copy ([`crate::MemoryMapSnapshot`]).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RetroMemoryDescriptor {
    pub flags: u64,
    pub ptr: *mut c_void,
    pub offset: usize,
    pub start: usize,
    pub select: usize,
    pub disconnect: usize,
    pub len: usize,
    pub addrspace: *const c_char,
}

/// `struct retro_memory_map`.
#[repr(C)]
pub struct RetroMemoryMap {
    pub descriptors: *const RetroMemoryDescriptor,
    pub num_descriptors: c_uint,
}

pub type RetroEnvironmentFn = unsafe extern "C" fn(cmd: c_uint, data: *mut c_void) -> bool;
pub type RetroVideoRefreshFn =
    unsafe extern "C" fn(data: *const c_void, width: c_uint, height: c_uint, pitch: usize);
pub type RetroAudioSampleFn = unsafe extern "C" fn(left: i16, right: i16);
pub type RetroAudioSampleBatchFn = unsafe extern "C" fn(data: *const i16, frames: usize) -> usize;
pub type RetroInputPollFn = unsafe extern "C" fn();
pub type RetroInputStateFn =
    unsafe extern "C" fn(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn struct_layouts_match_c() {
        let ptr = size_of::<usize>();
        assert_eq!(size_of::<RetroGameGeometry>(), 20);
        assert_eq!(size_of::<RetroSystemTiming>(), 16);
        // geometry (20) + padding to 8-byte alignment of the f64 timing block.
        assert_eq!(size_of::<RetroSystemAvInfo>(), 24 + 16);
        assert_eq!(align_of::<RetroSystemAvInfo>(), 8);
        assert_eq!(size_of::<RetroGameInfo>(), 4 * ptr);
        assert_eq!(size_of::<RetroSystemInfo>(), 4 * ptr);
        assert_eq!(size_of::<RetroLogCallback>(), ptr);
        // u64 flags + 7 pointer-sized fields; map = pointer + unsigned (padded).
        assert_eq!(size_of::<RetroMemoryDescriptor>(), 8 + 7 * ptr);
        assert_eq!(size_of::<RetroMemoryMap>(), 2 * ptr);
    }
}
