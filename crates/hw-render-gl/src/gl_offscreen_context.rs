//! [`GlOffscreenContext`]: a hidden SDL3 window owning an OpenGL context, plus one
//! framebuffer object (RGBA8 texture + 24/8 depth-stencil renderbuffer) sized to the
//! core's maximum, and a pixel-pack buffer for readback. Cores cache the framebuffer id
//! on the first `context_reset` (GLideN64's glsm), so it never changes for the session;
//! a larger maximum later (Flycast) re-allocates the storage behind it (`resize`).

use std::ffi::{c_void, CStr};

use glow::HasContext;
use libretro_host::{HwRenderContext, HwRenderRequest, VideoSink};
use sdl3::video::{GLContext, Window};
use sdl3::VideoSubsystem;

use crate::gl_context_setup::{create_targets, proc_address, set_attributes};
use crate::gl_frame_readback::read_frame;
use crate::gl_target_resize::resize_targets;

/// glsm (libretro-common) indexes a fixed table with framebuffer ids below this.
const GLSM_MAX_FRAMEBUFFERS: u32 = 128_000;

pub struct GlOffscreenContext {
    gl: glow::Context,
    fbo: glow::NativeFramebuffer,
    color: glow::NativeTexture,
    depth: glow::NativeRenderbuffer,
    pbo: glow::NativeBuffer,
    width: u32,
    height: u32,
    bottom_up: bool,
    // Drop order: GL objects are deleted in `Drop`, then the context, window, video.
    gl_ctx: GLContext,
    window: Window,
    _video: VideoSubsystem,
}

impl GlOffscreenContext {
    /// Creates the hidden window, context and framebuffer (`width × height`, the
    /// core's av maximum). The context is current on return.
    pub fn new(
        video: VideoSubsystem,
        request: &HwRenderRequest,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err(format!("invalid framebuffer size {width}x{height}"));
        }
        set_attributes(&video, request);
        let window = video
            .window("Nostyra OpenGL", 1, 1)
            .opengl()
            .hidden()
            .build()
            .map_err(|e| format!("cannot create the OpenGL window: {e}"))?;
        let gl_ctx = window
            .gl_create_context()
            .map_err(|e| format!("cannot create the OpenGL context: {e}"))?;
        window
            .gl_make_current(&gl_ctx)
            .map_err(|e| format!("cannot make the OpenGL context current: {e}"))?;
        // No swaps ever happen on this window; keep the driver from pacing anything.
        let _ = video.gl_set_swap_interval(sdl3::video::SwapInterval::Immediate);
        // SAFETY: the context is current on this thread; SDL resolves GL symbols
        // (with the opengl32 fallback for GL 1.1 entry points on Windows).
        let gl = unsafe { glow::Context::from_loader_function_cstr(proc_address) };
        // SAFETY: plain queries on the current context.
        let (version, renderer) = unsafe {
            (
                gl.get_parameter_string(glow::VERSION),
                gl.get_parameter_string(glow::RENDERER),
            )
        };
        tracing::info!(%version, %renderer, width, height, "OpenGL context created");
        if gl.version().major < 3 || (gl.version().major == 3 && gl.version().minor < 3) {
            return Err(format!("OpenGL 3.3 required, driver offers {version}"));
        }
        // SAFETY: the context is current; objects are owned by this struct.
        let (fbo, color, depth, pbo) = unsafe { create_targets(&gl, width, height)? };
        if fbo.0.get() >= GLSM_MAX_FRAMEBUFFERS {
            return Err(format!("framebuffer id {} too large", fbo.0.get()));
        }
        Ok(Self {
            gl,
            fbo,
            color,
            depth,
            pbo,
            width,
            height,
            bottom_up: request.bottom_left_origin,
            gl_ctx,
            window,
            _video: video,
        })
    }

    /// The loaded GL functions, for tests drawing into the framebuffer.
    pub fn gl(&self) -> &glow::Context {
        &self.gl
    }
}

impl HwRenderContext for GlOffscreenContext {
    fn make_current(&mut self) -> Result<(), String> {
        self.window
            .gl_make_current(&self.gl_ctx)
            .map_err(|e| e.to_string())
    }

    fn framebuffer_id(&self) -> usize {
        self.fbo.0.get() as usize
    }

    fn proc_address(&self, name: &CStr) -> *const c_void {
        proc_address(name)
    }

    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Err(format!("invalid framebuffer size {width}x{height}"));
        }
        // SAFETY: called inside a core call with this context current; the readback
        // buffer is only mapped inside `deliver_frame`, never across a core call.
        let (color, complete) = unsafe {
            resize_targets(
                &self.gl, self.fbo, self.color, self.depth, self.pbo, width, height,
            )?
        };
        self.color = color;
        if !complete {
            return Err(format!("framebuffer incomplete at {width}x{height}"));
        }
        (self.width, self.height) = (width, height);
        tracing::info!(width, height, "OpenGL framebuffer resized");
        Ok(())
    }

    fn deliver_frame(&mut self, width: u32, height: u32, sink: &mut dyn VideoSink) {
        let (w, h) = (width.min(self.width), height.min(self.height));
        if w == 0 || h == 0 {
            return;
        }
        // SAFETY: the session made this context current before `retro_run`, and the
        // core has returned; `fbo`/`pbo` are ours and sized for `w × h`.
        unsafe { read_frame(&self.gl, self.fbo, self.pbo, w, h, self.bottom_up, sink) };
    }
}

impl Drop for GlOffscreenContext {
    fn drop(&mut self) {
        if self.make_current().is_err() {
            return; // context unusable: the driver frees everything with it
        }
        // SAFETY: current context; the names were created by this context.
        unsafe {
            self.gl.delete_buffer(self.pbo);
            self.gl.delete_framebuffer(self.fbo);
            self.gl.delete_renderbuffer(self.depth);
            self.gl.delete_texture(self.color);
        }
    }
}

#[cfg(test)]
#[path = "gl_offscreen_context_tests.rs"]
mod tests;
