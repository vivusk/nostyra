//! Synchronous framebuffer readback: `glReadPixels` into a pixel-pack buffer, map it,
//! hand the rows to the sink, then restore every GL binding we touched (a pack buffer
//! left bound would redirect the core's own `glReadPixels` RDRAM copies).

use std::num::NonZeroU32;

use glow::HasContext;
use libretro_host::{PixelFormat, VideoFrame, VideoSink};

/// Reads `width × height` RGBA8 pixels from the bottom-left of `fbo` into `sink`.
///
/// # Safety
/// The GL context owning `fbo` and `pbo` must be current; `pbo` must hold at least
/// `width * height * 4` bytes.
pub(crate) unsafe fn read_frame(
    gl: &glow::Context,
    fbo: glow::NativeFramebuffer,
    pbo: glow::NativeBuffer,
    width: u32,
    height: u32,
    bottom_up: bool,
    sink: &mut dyn VideoSink,
) {
    let len = width as usize * height as usize * 4;
    // SAFETY: caller guarantees a current context and a large enough buffer.
    unsafe {
        let prev_read = gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING);
        let prev_pack = gl.get_parameter_i32(glow::PIXEL_PACK_BUFFER_BINDING);
        let prev_align = gl.get_parameter_i32(glow::PACK_ALIGNMENT);
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(fbo));
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 4);
        gl.bind_buffer(glow::PIXEL_PACK_BUFFER, Some(pbo));
        gl.read_pixels(
            0,
            0,
            width as i32,
            height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::BufferOffset(0),
        );
        let ptr = gl.map_buffer_range(glow::PIXEL_PACK_BUFFER, 0, len as i32, glow::MAP_READ_BIT);
        if ptr.is_null() {
            tracing::warn!("cannot map the readback buffer; frame dropped");
        } else {
            // SAFETY: the mapping covers `len` readable bytes until `unmap_buffer`.
            let data = std::slice::from_raw_parts(ptr, len);
            sink.frame(&VideoFrame {
                data,
                width,
                height,
                pitch: width as usize * 4,
                format: PixelFormat::Rgba8888,
                bottom_up,
            });
            gl.unmap_buffer(glow::PIXEL_PACK_BUFFER);
        }
        gl.bind_buffer(
            glow::PIXEL_PACK_BUFFER,
            gl_name(prev_pack).map(glow::NativeBuffer),
        );
        gl.bind_framebuffer(
            glow::READ_FRAMEBUFFER,
            gl_name(prev_read).map(glow::NativeFramebuffer),
        );
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, prev_align);
    }
}

/// A GL object name from an integer query (0 = none).
pub(crate) fn gl_name(raw: i32) -> Option<NonZeroU32> {
    NonZeroU32::new(raw as u32)
}
