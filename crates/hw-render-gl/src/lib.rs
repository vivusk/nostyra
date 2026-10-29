//! OpenGL backend for libretro hardware rendering ([`libretro_host::HwRenderProvider`]).
//!
//! A core such as Mupen64Plus-Next (GLideN64) renders into an OpenGL 3.3 core-profile
//! framebuffer object we own. The context lives on a hidden 1×1 SDL3 window that is
//! never shown or swapped, so the visible window keeps its pure wgpu swapchain and
//! fullscreen / resize never touch GL. After each frame the image is read back
//! (`glReadPixels` into a pixel-pack buffer) and handed to the usual `VideoSink`,
//! so presentation, shaders, screenshots and headless runs stay unchanged.
//!
//! **Unsafe exception**: like `libretro-host`, this crate may use `unsafe` (every glow
//! call is `unsafe`, as is resolving GL symbols for the core); each block carries a
//! `// SAFETY:` comment (docs/code-standards.md).

mod gl_context_probe;
mod gl_context_setup;
mod gl_frame_readback;
mod gl_offscreen_context;
mod gl_target_resize;
mod sdl_gl_provider;

pub use gl_context_probe::probe_gl_context;
pub use gl_offscreen_context::GlOffscreenContext;
pub use sdl_gl_provider::SdlGlProvider;
