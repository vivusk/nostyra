//! Hardware-render lifecycle order with a fake provider and fake core callbacks
//! (the session functions run on a raw context, as `CoreSession` drives them).

use std::ffi::{c_void, CString};
use std::sync::Mutex;

use super::*;
use crate::av_callbacks::video_refresh;
use crate::environment_callback::environment;
use crate::ffi_hw_render_types::*;
use crate::frame_sink_traits::{NullVideoSink, PixelFormat, VideoFrame};
use crate::host_context::set_current;
use crate::hw_render_test_fakes::{log, log_take, serial, FakeProvider, FAKE_FBO, FAKE_PROC};

type HostFns = (RetroHwGetCurrentFramebufferFn, RetroHwGetProcAddressFn);
/// Host functions handed to the fake core by `SET_HW_RENDER`.
pub(super) static HOST_FNS: Mutex<Option<HostFns>> = Mutex::new(None);

pub(super) fn host_fns() -> HostFns {
    HOST_FNS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .expect("SET_HW_RENDER accepted first")
}

/// Fake core `context_reset`: re-enters both host functions, like GLideN64's glsm.
pub(super) unsafe extern "C" fn fake_reset() {
    let (fb, proc_address) = host_fns();
    // SAFETY: host callbacks; the symbol is NUL-terminated.
    let (fb, ptr) = unsafe { (fb(), proc_address(c"glClear".as_ptr())) };
    log(format!("reset fb={fb} proc={}", ptr as usize == FAKE_PROC));
}

pub(super) unsafe extern "C" fn fake_destroy() {
    log("destroy");
}

/// Fake `retro_run`: reads the framebuffer id, then presents a HW frame.
pub(super) fn fake_run() {
    let (fb, _) = host_fns();
    // SAFETY: host callback.
    log(format!("run fb={}", unsafe { fb() }));
    // SAFETY: the HW marker is never dereferenced; pitch 0 as Mupen sends it.
    unsafe { video_refresh(RETRO_HW_FRAME_BUFFER_VALID as *const c_void, 640, 480, 0) };
}

pub(super) fn gl33_request() -> RetroHwRenderCallback {
    RetroHwRenderCallback {
        context_type: RETRO_HW_CONTEXT_OPENGL_CORE,
        context_reset: Some(fake_reset),
        get_current_framebuffer: None,
        get_proc_address: None,
        depth: true,
        stencil: false,
        bottom_left_origin: true,
        version_major: 3,
        version_minor: 3,
        cache_context: true,
        context_destroy: Some(fake_destroy),
        debug_context: false,
    }
}

/// Installs a fresh context as this thread's current one for `f`, then frees it.
fn with_raw_ctx(f: impl FnOnce(*mut HostContext)) {
    let raw = Box::into_raw(Box::new(HostContext::new(
        CString::default(),
        CString::default(),
    )));
    set_current(raw);
    f(raw);
    set_current(std::ptr::null_mut());
    // SAFETY: created by Box::into_raw above and no longer installed.
    drop(unsafe { Box::from_raw(raw) });
}

/// `SET_HW_RENDER` through the real environment handler, with `provider` installed.
fn accept_request(ctx: *mut HostContext, provider: &FakeProvider) -> ProviderGuard {
    let guard = ProviderGuard::install(ctx, Some(provider));
    let mut callback = gl33_request();
    // SAFETY: `struct retro_hw_render_callback *`.
    assert!(unsafe { environment(RETRO_ENVIRONMENT_SET_HW_RENDER, (&raw mut callback).cast()) });
    *HOST_FNS.lock().unwrap_or_else(|e| e.into_inner()) = Some((
        callback.get_current_framebuffer.expect("filled"),
        callback.get_proc_address.expect("filled"),
    ));
    guard
}

#[test]
fn lifecycle_runs_in_libretro_order_with_reentrant_host_fns() {
    let _serial = serial();
    let provider = FakeProvider::default();
    let mut sink = NullVideoSink::default();
    with_raw_ctx(|ctx| {
        let guard = accept_request(ctx, &provider);
        create_and_reset(ctx, 640, 480).expect("context created");
        drop(guard);
        // SAFETY: owned test context; no callback runs.
        assert!(unsafe { (*ctx).hw_provider.is_none() });
        make_current(ctx);
        fake_run();
        deliver_frame(ctx, &mut sink);
        deliver_frame(ctx, &mut sink); // no new frame: nothing delivered
        destroy(ctx);
        destroy(ctx); // idempotent: destroy is owed once
        log("unload");
        log("deinit");
    });
    assert_eq!(
        log_take(),
        [
            "create 640x480",
            "current",
            "proc glClear",
            &format!("reset fb={FAKE_FBO} proc=true"),
            "current",
            &format!("run fb={FAKE_FBO}"),
            "deliver 640x480",
            "current",
            "destroy",
            "current",
            "unload",
            "deinit",
            "drop context",
        ]
    );
    // The bottom-up frame was hashed in display order (top row red first).
    let mut top_down = NullVideoSink::default();
    top_down.frame(&VideoFrame {
        data: &[255, 0, 0, 0, 0, 0, 255, 0],
        width: 1,
        height: 2,
        pitch: 4,
        format: PixelFormat::Rgba8888,
        bottom_up: false,
    });
    assert_eq!(sink.frames, 1);
    assert_eq!(sink.last_checksum, top_down.last_checksum);
}

#[test]
fn failed_create_reports_graphics_error_and_never_destroys() {
    let _serial = serial();
    let provider = FakeProvider { fail_create: true };
    with_raw_ctx(|ctx| {
        let guard = accept_request(ctx, &provider);
        let error = create_and_reset(ctx, 640, 480).expect_err("create fails");
        assert!(matches!(error, HostError::HwRenderUnavailable(ref r) if r.contains("3.3")));
        drop(guard);
        destroy(ctx);
    });
    assert_eq!(log_take(), ["create 640x480"]);
}

#[test]
fn software_session_creates_nothing_and_ignores_the_hw_marker() {
    let _serial = serial();
    let mut sink = NullVideoSink::default();
    with_raw_ctx(|ctx| {
        let guard = ProviderGuard::install(ctx, Some(&FakeProvider::default()));
        create_and_reset(ctx, 640, 480).expect("nothing to create");
        drop(guard);
        // SAFETY: marker only; never dereferenced.
        unsafe { video_refresh(RETRO_HW_FRAME_BUFFER_VALID as *const c_void, 640, 480, 0) };
        deliver_frame(ctx, &mut sink);
        destroy(ctx);
    });
    assert!(log_take().is_empty());
    assert_eq!(sink.frames, 0);
}

#[test]
fn refused_request_turns_a_rejected_load_into_a_graphics_error() {
    let _serial = serial();
    with_raw_ctx(|ctx| {
        // SAFETY: owned test context; no callback runs.
        unsafe { (*ctx).hw_refused = Some("no graphics backend for Vulkan".into()) };
        let error = load_failure(ctx, HostError::LoadGameRejected(None));
        assert!(matches!(error, HostError::HwRenderUnavailable(ref r) if r.contains("Vulkan")));
        let error = load_failure(ctx, HostError::LoadGameRejected(None));
        assert!(matches!(error, HostError::LoadGameRejected(None)));
    });
}
