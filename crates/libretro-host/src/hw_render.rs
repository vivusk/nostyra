//! Backend-agnostic hardware-render API.
//!
//! A core asks for a graphics context during `retro_load_game` (`SET_HW_RENDER`).
//! The host knows nothing about OpenGL or Vulkan: it asks a [`HwRenderProvider`]
//! whether it can make that kind of context, and after `retro_get_system_av_info`
//! asks it to build one ([`HwRenderContext`]). The session owns the context, keeps
//! it current around every core entry point and hands finished frames to the usual
//! [`VideoSink`] through [`HwRenderContext::deliver_frame`].
//!
//! Backends live outside this crate (OpenGL: `hw-render-gl`). A Vulkan backend adds
//! a negotiation hook later (`SET_HW_RENDER_CONTEXT_NEGOTIATION_INTERFACE`), and a
//! zero-copy backend can hand frames to a GPU-side sink through the same
//! `deliver_frame` seam.

use std::ffi::{c_void, CStr};

use crate::frame_sink_traits::VideoSink;

/// Graphics API a core asks for (`enum retro_hw_context_type` + version).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HwContextKind {
    /// Compatibility-profile OpenGL (any 2.x+).
    OpenGl,
    /// Core-profile OpenGL of at least this version.
    OpenGlCore {
        major: u32,
        minor: u32,
    },
    OpenGlEs2,
    OpenGlEs3,
    Vulkan,
    /// Any other `retro_hw_context_type` value (Direct3D, GLES version, ...).
    Other(u32),
}

/// Everything the core asked for in `SET_HW_RENDER`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HwRenderRequest {
    pub kind: HwContextKind,
    /// The framebuffer needs a depth attachment.
    pub depth: bool,
    /// The framebuffer needs a stencil attachment.
    pub stencil: bool,
    /// Row 0 of the core's image is the bottom row (OpenGL convention).
    pub bottom_left_origin: bool,
    /// The core prefers the context to survive video re-inits (we never re-init).
    pub cache_context: bool,
    /// The core asked for a debug context.
    pub debug: bool,
}

/// Builds graphics contexts for cores. Implemented by a backend crate and passed in
/// through `SessionConfig::hw_render`; it only has to live while the session loads.
pub trait HwRenderProvider: std::fmt::Debug {
    /// Can this provider build a context for `request`? Called inside
    /// `retro_load_game`: must be cheap and must not initialise anything.
    fn supports(&self, request: &HwRenderRequest) -> bool;
    /// Answer to `GET_PREFERRED_HW_RENDER`.
    fn preferred(&self) -> HwContextKind;
    /// Builds the context and a framebuffer of at least `max_width × max_height`
    /// (the core's `retro_get_system_av_info` maximum). Called once, after load.
    fn create(
        &self,
        request: &HwRenderRequest,
        max_width: u32,
        max_height: u32,
    ) -> Result<Box<dyn HwRenderContext>, String>;
}

/// A live graphics context owned by the session for its whole lifetime.
pub trait HwRenderContext {
    /// Makes the context current on this thread (before every core entry point).
    fn make_current(&mut self) -> Result<(), String>;
    /// The framebuffer the core renders to (`get_current_framebuffer`). Must stay
    /// the same for the whole session: cores cache it on the first `context_reset`.
    fn framebuffer_id(&self) -> usize;
    /// Resolves an API symbol for the core (`get_proc_address`); null if unknown.
    fn proc_address(&self, name: &CStr) -> *const c_void;
    /// Current framebuffer size (the allocated maximum frame size).
    fn size(&self) -> (u32, u32);
    /// Re-allocates the framebuffer's storage at `width × height` (a core announced a
    /// larger maximum through `SET_SYSTEM_AV_INFO`). The framebuffer id must stay the
    /// same (cores cache it). Called with the context current, inside a core call.
    fn resize(&mut self, width: u32, height: u32) -> Result<(), String>;
    /// The core finished a `width × height` frame in the framebuffer: hand it to
    /// `sink` (a CPU readback today; a GPU-side sink in a zero-copy backend).
    fn deliver_frame(&mut self, width: u32, height: u32, sink: &mut dyn VideoSink);
}
