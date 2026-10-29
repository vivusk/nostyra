//! Hand-written libretro.h declarations for hardware-rendered cores
//! (`SET_HW_RENDER` and friends). Field order matches libretro.h exactly.

use std::ffi::{c_char, c_uint, c_void};

pub(crate) const RETRO_ENVIRONMENT_SET_HW_RENDER: c_uint = 14;
pub(crate) const RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER: c_uint = 56;
// Known but unanswered for now (Vulkan / shared contexts come with a later backend).
#[allow(dead_code)]
pub(crate) const RETRO_ENVIRONMENT_GET_HW_RENDER_INTERFACE: c_uint = 41;
#[allow(dead_code)]
pub(crate) const RETRO_ENVIRONMENT_SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE: c_uint = 43;
#[allow(dead_code)]
pub(crate) const RETRO_ENVIRONMENT_SET_HW_SHARED_CONTEXT: c_uint = 44;

/// `enum retro_hw_context_type`.
pub(crate) const RETRO_HW_CONTEXT_OPENGL: c_uint = 1;
pub(crate) const RETRO_HW_CONTEXT_OPENGLES2: c_uint = 2;
pub(crate) const RETRO_HW_CONTEXT_OPENGL_CORE: c_uint = 3;
pub(crate) const RETRO_HW_CONTEXT_OPENGLES3: c_uint = 4;
pub(crate) const RETRO_HW_CONTEXT_VULKAN: c_uint = 6;

/// `video_refresh` data value meaning "the frame is in the HW framebuffer"
/// (`RETRO_HW_FRAME_BUFFER_VALID`, `(void*)-1`). Never dereferenced.
pub(crate) const RETRO_HW_FRAME_BUFFER_VALID: usize = usize::MAX;

/// `retro_hw_context_reset_t`: core callback without arguments.
pub(crate) type RetroHwContextResetFn = unsafe extern "C" fn();
/// `retro_hw_get_current_framebuffer_t`: frontend returns the FBO the core draws to.
pub(crate) type RetroHwGetCurrentFramebufferFn = unsafe extern "C" fn() -> usize;
/// `retro_hw_get_proc_address_t`: frontend resolves a GL symbol (returns a
/// `retro_proc_address_t`, passed through as an opaque pointer).
pub(crate) type RetroHwGetProcAddressFn = unsafe extern "C" fn(sym: *const c_char) -> *const c_void;

/// `struct retro_hw_render_callback`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct RetroHwRenderCallback {
    pub context_type: c_uint,
    pub context_reset: Option<RetroHwContextResetFn>,
    pub get_current_framebuffer: Option<RetroHwGetCurrentFramebufferFn>,
    pub get_proc_address: Option<RetroHwGetProcAddressFn>,
    pub depth: bool,
    pub stencil: bool,
    pub bottom_left_origin: bool,
    pub version_major: c_uint,
    pub version_minor: c_uint,
    pub cache_context: bool,
    pub context_destroy: Option<RetroHwContextResetFn>,
    pub debug_context: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    /// Offsets of the C struct on 64-bit targets (bools are 1 byte, enum/unsigned 4).
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn hw_render_callback_matches_libretro_h_layout() {
        assert_eq!(offset_of!(RetroHwRenderCallback, context_reset), 8);
        assert_eq!(offset_of!(RetroHwRenderCallback, get_proc_address), 24);
        assert_eq!(offset_of!(RetroHwRenderCallback, depth), 32);
        assert_eq!(offset_of!(RetroHwRenderCallback, bottom_left_origin), 34);
        assert_eq!(offset_of!(RetroHwRenderCallback, version_major), 36);
        assert_eq!(offset_of!(RetroHwRenderCallback, cache_context), 44);
        assert_eq!(offset_of!(RetroHwRenderCallback, context_destroy), 48);
        assert_eq!(offset_of!(RetroHwRenderCallback, debug_context), 56);
        assert_eq!(size_of::<RetroHwRenderCallback>(), 64);
    }
}
