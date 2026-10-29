//! Growing the framebuffer mid-session (`SET_SYSTEM_AV_INFO` with a larger maximum,
//! sent by Flycast inside `retro_run`). The framebuffer object keeps its id (cores
//! cache it); its colour texture is replaced (immutable storage cannot change size),
//! the depth/stencil renderbuffer and the readback buffer get new storage. Every
//! binding touched is restored, because the core is in the middle of its frame.

use glow::HasContext;

use crate::gl_frame_readback::gl_name;

/// Re-allocates the attachments of `fbo` and the readback buffer at `width × height`.
/// Returns the new colour texture (now attached; the old one is deleted) and whether
/// the framebuffer is complete. `Err` only if no texture could be created (nothing
/// changed then).
///
/// # Safety
/// The GL context owning every name must be current; `pbo` must not be mapped.
pub(crate) unsafe fn resize_targets(
    gl: &glow::Context,
    fbo: glow::NativeFramebuffer,
    old_color: glow::NativeTexture,
    depth: glow::NativeRenderbuffer,
    pbo: glow::NativeBuffer,
    width: u32,
    height: u32,
) -> Result<(glow::NativeTexture, bool), String> {
    let (w, h) = (width as i32, height as i32);
    // SAFETY: caller guarantees a current context; names belong to it.
    unsafe {
        let prev_draw = gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING);
        let prev_read = gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING);
        let prev_tex = gl.get_parameter_i32(glow::TEXTURE_BINDING_2D);
        let prev_rb = gl.get_parameter_i32(glow::RENDERBUFFER_BINDING);
        let prev_pack = gl.get_parameter_i32(glow::PIXEL_PACK_BUFFER_BINDING);

        let color = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(color));
        gl.tex_storage_2d(glow::TEXTURE_2D, 1, glow::RGBA8, w, h);
        gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
        gl.renderbuffer_storage(glow::RENDERBUFFER, glow::DEPTH24_STENCIL8, w, h);
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(color),
            0,
        );
        // Re-attach: the storage behind the renderbuffer name changed.
        gl.framebuffer_renderbuffer(
            glow::FRAMEBUFFER,
            glow::DEPTH_STENCIL_ATTACHMENT,
            glow::RENDERBUFFER,
            Some(depth),
        );
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(pbo));
        gl.buffer_data_size(glow::PIXEL_PACK_BUFFER, w * h * 4, glow::STREAM_READ);

        gl.bind_buffer(
            glow::PIXEL_PACK_BUFFER,
            gl_name(prev_pack).map(glow::NativeBuffer),
        );
        gl.bind_renderbuffer(
            glow::RENDERBUFFER,
            gl_name(prev_rb).map(glow::NativeRenderbuffer),
        );
        gl.bind_texture(glow::TEXTURE_2D, gl_name(prev_tex).map(glow::NativeTexture));
        gl.bind_framebuffer(
            glow::DRAW_FRAMEBUFFER,
            gl_name(prev_draw).map(glow::NativeFramebuffer),
        );
        gl.bind_framebuffer(
            glow::READ_FRAMEBUFFER,
            gl_name(prev_read).map(glow::NativeFramebuffer),
        );
        gl.delete_texture(old_color);
        Ok((color, status == glow::FRAMEBUFFER_COMPLETE))
    }
}
