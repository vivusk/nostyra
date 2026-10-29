//! libretro.h subset for core options (`GET_VARIABLE`), disk control and core
//! messages (kept apart from `ffi_types` for size). Same rules: hand-written,
//! layout checked by tests.

use std::ffi::{c_char, c_int, c_uint, c_void};

pub const RETRO_ENVIRONMENT_SET_MESSAGE: c_uint = 6;
pub const RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE: c_uint = 13;
pub const RETRO_ENVIRONMENT_GET_DISK_CONTROL_INTERFACE_VERSION: c_uint = 57;
pub const RETRO_ENVIRONMENT_SET_DISK_CONTROL_EXT_INTERFACE: c_uint = 58;
pub const RETRO_ENVIRONMENT_GET_MESSAGE_INTERFACE_VERSION: c_uint = 59;
pub const RETRO_ENVIRONMENT_SET_MESSAGE_EXT: c_uint = 60;

/// `retro_message_target`: only shown in the log, never on screen.
pub const RETRO_MESSAGE_TARGET_LOG: c_int = 2;

/// `struct retro_variable` (`GET_VARIABLE` in/out).
#[repr(C)]
pub struct RetroVariable {
    pub key: *const c_char,
    pub value: *const c_char,
}

/// `struct retro_message` (legacy `SET_MESSAGE`).
#[repr(C)]
pub struct RetroMessage {
    pub msg: *const c_char,
    pub frames: c_uint,
}

/// `struct retro_message_ext` (`SET_MESSAGE_EXT`).
#[repr(C)]
pub struct RetroMessageExt {
    pub msg: *const c_char,
    pub duration: c_uint,
    pub priority: c_uint,
    pub level: c_int,
    pub target: c_int,
    pub kind: c_int,
    pub progress: i8,
}

pub type SetEjectStateFn = unsafe extern "C" fn(ejected: bool) -> bool;
pub type GetEjectStateFn = unsafe extern "C" fn() -> bool;
pub type GetImageIndexFn = unsafe extern "C" fn() -> c_uint;
pub type SetImageIndexFn = unsafe extern "C" fn(index: c_uint) -> bool;
pub type GetNumImagesFn = unsafe extern "C" fn() -> c_uint;
pub type ReplaceImageIndexFn = unsafe extern "C" fn(index: c_uint, info: *const c_void) -> bool;
pub type AddImageIndexFn = unsafe extern "C" fn() -> bool;
pub type SetInitialImageFn = unsafe extern "C" fn(index: c_uint, path: *const c_char) -> bool;
pub type GetImageTextFn = unsafe extern "C" fn(index: c_uint, out: *mut c_char, len: usize) -> bool;

/// `struct retro_disk_control_callback` (legacy interface: the first 7 entries of
/// the EXT table).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RetroDiskControlCallback {
    pub set_eject_state: Option<SetEjectStateFn>,
    pub get_eject_state: Option<GetEjectStateFn>,
    pub get_image_index: Option<GetImageIndexFn>,
    pub set_image_index: Option<SetImageIndexFn>,
    pub get_num_images: Option<GetNumImagesFn>,
    pub replace_image_index: Option<ReplaceImageIndexFn>,
    pub add_image_index: Option<AddImageIndexFn>,
}

/// `struct retro_disk_control_ext_callback`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RetroDiskControlExtCallback {
    pub set_eject_state: Option<SetEjectStateFn>,
    pub get_eject_state: Option<GetEjectStateFn>,
    pub get_image_index: Option<GetImageIndexFn>,
    pub set_image_index: Option<SetImageIndexFn>,
    pub get_num_images: Option<GetNumImagesFn>,
    pub replace_image_index: Option<ReplaceImageIndexFn>,
    pub add_image_index: Option<AddImageIndexFn>,
    pub set_initial_image: Option<SetInitialImageFn>,
    pub get_image_path: Option<GetImageTextFn>,
    pub get_image_label: Option<GetImageTextFn>,
}

impl From<RetroDiskControlCallback> for RetroDiskControlExtCallback {
    fn from(legacy: RetroDiskControlCallback) -> Self {
        Self {
            set_eject_state: legacy.set_eject_state,
            get_eject_state: legacy.get_eject_state,
            get_image_index: legacy.get_image_index,
            set_image_index: legacy.set_image_index,
            get_num_images: legacy.get_num_images,
            replace_image_index: legacy.replace_image_index,
            add_image_index: legacy.add_image_index,
            set_initial_image: None,
            get_image_path: None,
            get_image_label: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn struct_layouts_match_c() {
        let ptr = size_of::<usize>();
        assert_eq!(size_of::<RetroVariable>(), 2 * ptr);
        assert_eq!(size_of::<RetroMessage>(), 2 * ptr);
        // msg + 5 × 32-bit fields + int8 progress, padded to pointer alignment.
        assert_eq!(size_of::<RetroMessageExt>(), ptr + 24);
        assert_eq!(size_of::<RetroDiskControlCallback>(), 7 * ptr);
        assert_eq!(size_of::<RetroDiskControlExtCallback>(), 10 * ptr);
    }
}
