//! Context setup helpers for [`crate::GlOffscreenContext`]: SDL GL attributes, symbol
//! lookup, and the one-time framebuffer / readback-buffer allocation.

use std::ffi::{c_void, CStr};

use glow::HasContext;
use libretro_host::{HwContextKind, HwRenderRequest};
use sdl3::video::GLProfile;
use sdl3::VideoSubsystem;

/// SDL GL attributes for the next window/context (global SDL state).
pub(crate) fn set_attributes(video: &VideoSubsystem, request: &HwRenderRequest) {
    let attr = video.gl_attr();
    match request.kind {
        HwContextKind::OpenGl => attr.set_context_profile(GLProfile::Compatibility),
        _ => {
            attr.set_context_profile(GLProfile::Core);
            attr.set_context_version(3, 3);
        }
    }
    attr.set_double_buffer(false);
    attr.set_depth_size(0);
    attr.set_stencil_size(0);
    attr.set_framebuffer_srgb_compatible(false);
    let mut flags = attr.set_context_flags();
    if cfg!(target_os = "macos") && request.kind != HwContextKind::OpenGl {
        flags.forward_compatible();
    }
    if request.debug {
        flags.debug();
    }
    flags.set();
}

/// `SDL_GL_GetProcAddress` as a plain pointer (null if unknown).
pub(crate) fn proc_address(name: &CStr) -> *const c_void {
    // SAFETY: NUL-terminated name; SDL video is initialised and a context exists.
    let f = unsafe { sdl3::sys::video::SDL_GL_GetProcAddress(name.as_ptr()) };
    f.map_or(std::ptr::null(), |f| f as *const c_void)
}

/// Framebuffer (colour texture + depth/stencil renderbuffer) and the readback buffer.
///
/// # Safety
/// A GL context must be current on this thread.
pub(crate) unsafe fn create_targets(
    gl: &glow::Context,
    width: u32,
    height: u32,
) -> Result<
    (
        glow::NativeFramebuffer,
        glow::NativeTexture,
        glow::NativeRenderbuffer,
        glow::NativeBuffer,
    ),
    String,
> {
    let (w, h) = (width as i32, height as i32);
    // SAFETY: caller guarantees a current context; all names are freshly created.
    unsafe {
        let color = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(color));
        gl.tex_storage_2d(glow::TEXTURE_2D, 1, glow::RGBA8, w, h);
        gl.bind_texture(glow::TEXTURE_2D, None);
        let depth = gl.create_renderbuffer()?;
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
        gl.renderbuffer_storage(glow::RENDERBUFFER, glow::DEPTH24_STENCIL8, w, h);
        gl.bind_renderbuffer(glow::RENDERBUFFER, None);
        let fbo = gl.create_framebuffer()?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(color),
            0,
        );
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::DEPTH_STENCIL_ATTACHMENT,
            glow::RENDERBUFFER,
            Some(depth),
        );
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        if status != glow::FRAMEBUFFER_COMPLETE {
            return Err(format!("framebuffer incomplete (0x{status:X})"));
        }
        gl.viewport(0, 0, w, h);
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT | glow::STENCIL_BUFFER_BIT);
        // Left bound: the core's first bind of "framebuffer 0" maps to it anyway.
        let pbo = gl.create_buffer()?;
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(pbo));
        gl.buffer_data_size(glow::PIXEL_PACK_BUFFER, w * h * 4, glow::STREAM_READ);
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, None);
        Ok((fbo, color, depth, pbo))
    }
}
