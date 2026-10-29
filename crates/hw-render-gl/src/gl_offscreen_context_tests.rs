//! Real-GL tests. Skipped (with a note) when no OpenGL 3.3 context can be created,
//! unless `NOSTYRA_REQUIRE_GL=1` turns the skip into a failure. Not on macOS (SDL
//! windows only on the main thread there; test threads are not). One at a time:
//! SDL windows and GL attributes are process-global.

use std::sync::Mutex;

use glow::HasContext;
use libretro_host::{HwContextKind, HwRenderProvider, VideoFrame};

use super::*;
use crate::sdl_gl_provider::SdlGlProvider;

static GL_LOCK: Mutex<()> = Mutex::new(());

/// The GL gate: `Some(lock)` when GL is usable, `None` to skip.
fn gl_gate() -> Option<std::sync::MutexGuard<'static, ()>> {
    let guard = GL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let required = std::env::var("NOSTYRA_REQUIRE_GL").is_ok_and(|v| v == "1");
    if cfg!(target_os = "macos") {
        assert!(!required, "GL tests cannot run on macOS test threads");
        eprintln!("skipped: SDL windows need the main thread on macOS");
        return None;
    }
    match crate::probe_gl_context() {
        Ok(()) => Some(guard),
        Err(error) if required => panic!("NOSTYRA_REQUIRE_GL=1 but no OpenGL 3.3: {error}"),
        Err(error) => {
            eprintln!("skipped: no OpenGL 3.3 context ({error})");
            None
        }
    }
}

fn gl33_request() -> HwRenderRequest {
    HwRenderRequest {
        kind: HwContextKind::OpenGlCore { major: 3, minor: 3 },
        depth: true,
        stencil: false,
        bottom_left_origin: true,
        cache_context: true,
        debug: false,
    }
}

/// Keeps the last frame's rows in display order (top first).
#[derive(Default)]
struct CaptureSink {
    rows: Vec<Vec<u8>>,
}

impl VideoSink for CaptureSink {
    fn frame(&mut self, frame: &VideoFrame<'_>) {
        self.rows = (0..frame.height).map(|y| frame.row(y).to_vec()).collect();
    }

    fn dupe(&mut self, _width: u32, _height: u32) {}
}

#[test]
fn framebuffer_is_stable_sized_to_max_and_glsm_safe() {
    let Some(_gl) = gl_gate() else { return };
    let provider = SdlGlProvider::new();
    let mut context = provider.create(&gl33_request(), 640, 480).expect("context");
    assert!(provider.video_initialised());
    let id = context.framebuffer_id();
    assert!(id > 0 && id < GLSM_MAX_FRAMEBUFFERS as usize);
    context.make_current().expect("current");
    assert_eq!(context.framebuffer_id(), id);
    // Symbols resolve, including GL 1.1 ones (opengl32 fallback on Windows).
    for name in [c"glClear", c"glReadPixels", c"glBindFramebuffer"] {
        assert!(!context.proc_address(name).is_null(), "{name:?}");
    }
}

#[test]
fn readback_is_upright_opaque_and_restores_gl_bindings() {
    let Some(_gl) = gl_gate() else { return };
    let video = sdl3::init().and_then(|s| s.video()).expect("video");
    let mut context = GlOffscreenContext::new(video, &gl33_request(), 64, 32).expect("ctx");
    assert_eq!(context.size(), (64, 32));
    let gl = context.gl();
    // GL origin is bottom-left: rows 16..32 are the top of the image → red; bottom blue.
    // SAFETY: the context is current (just created on this thread).
    let fbo = unsafe {
        let fbo = gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING);
        gl.enable(glow::SCISSOR_TEST);
        gl.scissor(0, 16, 64, 16);
        gl.clear_color(1.0, 0.0, 0.0, 0.0); // alpha 0: must come out opaque later
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.scissor(0, 0, 64, 16);
        gl.clear_color(0.0, 0.0, 1.0, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.disable(glow::SCISSOR_TEST);
        gl.pixel_store_i32(glow::PACK_ALIGNMENT, 1);
        fbo
    };
    let mut sink = CaptureSink::default();
    context.deliver_frame(64, 32, &mut sink);
    assert_eq!(sink.rows.len(), 32);
    assert_eq!(
        &sink.rows[0][..4],
        &[255, 0, 0, 0],
        "top row red (alpha raw)"
    );
    assert_eq!(&sink.rows[31][..4], &[0, 0, 255, 255], "bottom row blue");
    let gl = context.gl();
    // SAFETY: plain queries on the current context.
    unsafe {
        assert_eq!(gl.get_parameter_i32(glow::PIXEL_PACK_BUFFER_BINDING), 0);
        assert_eq!(gl.get_parameter_i32(glow::READ_FRAMEBUFFER_BINDING), fbo);
        assert_eq!(gl.get_parameter_i32(glow::PACK_ALIGNMENT), 1);
    }
}

#[test]
fn resize_keeps_the_framebuffer_id_reads_back_upright_and_restores_bindings() {
    let Some(_gl) = gl_gate() else { return };
    let video = sdl3::init().and_then(|s| s.video()).expect("video");
    let mut context = GlOffscreenContext::new(video, &gl33_request(), 64, 32).expect("ctx");
    let id = context.framebuffer_id();
    let gl = context.gl();
    // The core's own state mid-frame: another texture bound, default framebuffer.
    // SAFETY: the context is current (just created on this thread).
    let core_tex = unsafe {
        let tex = gl.create_texture().expect("texture");
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        tex
    };
    context.resize(128, 96).expect("resize");
    assert_eq!(context.framebuffer_id(), id, "cores cache the id");
    assert_eq!(context.size(), (128, 96));
    let gl = context.gl();
    // SAFETY: plain queries and draws on the current context.
    unsafe {
        assert_eq!(
            gl.get_parameter_i32(glow::TEXTURE_BINDING_2D),
            core_tex.0.get() as i32
        );
        assert_eq!(gl.get_parameter_i32(glow::DRAW_FRAMEBUFFER_BINDING), 0);
        assert_eq!(gl.get_parameter_i32(glow::PIXEL_PACK_BUFFER_BINDING), 0);
        gl.bind_framebuffer(
            glow::FRAMEBUFFER,
            std::num::NonZeroU32::new(id as u32).map(glow::NativeFramebuffer),
        );
        gl.viewport(0, 0, 128, 96);
        gl.enable(glow::SCISSOR_TEST);
        gl.scissor(0, 48, 128, 48);
        gl.clear_color(1.0, 0.0, 0.0, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.scissor(0, 0, 128, 48);
        gl.clear_color(0.0, 0.0, 1.0, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.disable(glow::SCISSOR_TEST);
    }
    let mut sink = CaptureSink::default();
    context.deliver_frame(128, 96, &mut sink);
    assert_eq!(sink.rows.len(), 96, "not cropped to the old 32 rows");
    assert_eq!(&sink.rows[0][..4], &[255, 0, 0, 255], "top row red");
    assert_eq!(
        &sink.rows[95][508..512],
        &[0, 0, 255, 255],
        "bottom-right blue"
    );
    assert!(context.resize(0, 10).is_err());
    assert_eq!(context.size(), (128, 96));
}
