//! rcheevos v12.5.0 `rc_libretro_*` declarations (`src/rc_libretro.h`): memory regions
//! over a core's `SET_MEMORY_MAPS` / `retro_get_memory_data`, and the disallowed core
//! option tables. Safe wrappers: `raw_memory_regions`, `option_rules`.

use std::ffi::{c_char, c_int, c_uint};

use libretro_host::RetroMemoryDescriptor;

/// `RC_LIBRETRO_MAX_MEMORY_REGIONS`.
pub const MAX_MEMORY_REGIONS: usize = 32;

/// `rc_libretro_memory_regions_t`.
#[repr(C)]
pub struct RcLibretroMemoryRegions {
    pub data: [*mut u8; MAX_MEMORY_REGIONS],
    pub size: [usize; MAX_MEMORY_REGIONS],
    pub total_size: usize,
    pub count: u32,
}

/// `rc_libretro_core_memory_info_t`.
#[repr(C)]
pub struct RcLibretroCoreMemoryInfo {
    pub data: *mut u8,
    pub size: usize,
}

/// `struct retro_memory_map` over libretro-host's descriptor copy (layout checked by
/// `ffi_layout_tests`).
#[repr(C)]
pub struct RetroMemoryMap {
    pub descriptors: *const RetroMemoryDescriptor,
    pub num_descriptors: c_uint,
}

/// `rc_disallowed_setting_t` (array ends with a NULL `setting`).
#[repr(C)]
pub struct RcDisallowedSetting {
    pub setting: *const c_char,
    pub value: *const c_char,
}

/// No userdata: `raw_memory_regions` hands the core buffers over through a scoped
/// thread-local.
pub type GetCoreMemoryInfoFn = unsafe extern "C" fn(id: u32, info: *mut RcLibretroCoreMemoryInfo);
pub type LibretroMessageFn = unsafe extern "C" fn(message: *const c_char);

unsafe extern "C" {
    pub fn rc_libretro_memory_init(
        regions: *mut RcLibretroMemoryRegions,
        mmap: *const RetroMemoryMap,
        get_core_memory_info: GetCoreMemoryInfoFn,
        console_id: u32,
    ) -> c_int;
    pub fn rc_libretro_memory_read(
        regions: *const RcLibretroMemoryRegions,
        address: u32,
        buffer: *mut u8,
        num_bytes: u32,
    ) -> u32;
    pub fn rc_libretro_memory_destroy(regions: *mut RcLibretroMemoryRegions);
    pub fn rc_libretro_init_verbose_message_callback(callback: Option<LibretroMessageFn>);

    pub fn rc_libretro_get_disallowed_settings(
        library_name: *const c_char,
    ) -> *const RcDisallowedSetting;
    pub fn rc_libretro_is_setting_allowed(
        list: *const RcDisallowedSetting,
        setting: *const c_char,
        value: *const c_char,
    ) -> c_int;
}
