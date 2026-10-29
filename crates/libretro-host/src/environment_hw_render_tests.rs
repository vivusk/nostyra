//! `SET_HW_RENDER` / `GET_PREFERRED_HW_RENDER` against a fake provider.

use std::ffi::{c_uint, c_void};

use super::*;
use crate::environment_callback::environment;
use crate::host_context::run_with_test_ctx;
use crate::hw_render::HwRenderProvider;
use crate::hw_render_test_fakes::{serial, FakeProvider};

unsafe extern "C" fn noop() {}

fn request(context_type: c_uint, major: c_uint, minor: c_uint) -> RetroHwRenderCallback {
    RetroHwRenderCallback {
        context_type,
        context_reset: Some(noop),
        get_current_framebuffer: None,
        get_proc_address: None,
        depth: true,
        stencil: false,
        bottom_left_origin: true,
        version_major: major,
        version_minor: minor,
        cache_context: true,
        context_destroy: Some(noop),
        debug_context: false,
    }
}

fn env(cmd: c_uint, data: *mut c_void) -> bool {
    // SAFETY: tests pass a pointer of the type documented for `cmd`.
    unsafe { environment(cmd, data) }
}

/// Runs `f` with `provider` installed as the session's provider.
fn with_provider(provider: Option<&FakeProvider>, f: impl FnOnce()) -> HostContext {
    let raw = provider.map(|p| {
        let short: *const (dyn HwRenderProvider + '_) = p;
        // SAFETY: test-only lifetime extension; the context is gone before `p`.
        unsafe {
            std::mem::transmute::<
                *const (dyn HwRenderProvider + '_),
                *const (dyn HwRenderProvider + 'static),
            >(short)
        }
    });
    run_with_test_ctx(|ctx| ctx.hw_provider = raw, f)
}

#[test]
fn gl_core_33_request_is_accepted_and_host_fns_are_filled() {
    let _serial = serial();
    let provider = FakeProvider::default();
    let mut cb = request(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 3);
    let ctx = with_provider(Some(&provider), || {
        assert!(env(RETRO_ENVIRONMENT_SET_HW_RENDER, (&raw mut cb).cast()));
    });
    assert!(cb.get_current_framebuffer.is_some() && cb.get_proc_address.is_some());
    let state = ctx.hw_request.expect("request stored");
    assert_eq!(
        state.request.kind,
        HwContextKind::OpenGlCore { major: 3, minor: 3 }
    );
    assert!(state.request.depth && state.request.bottom_left_origin && !state.request.stencil);
    assert!(state.context_reset.is_some() && state.context_destroy.is_some());
    assert!(ctx.hw_refused.is_none());
}

#[test]
fn unsupported_kind_or_no_provider_is_refused_with_a_reason() {
    let _serial = serial();
    let provider = FakeProvider::default();
    let mut vulkan = request(RETRO_HW_CONTEXT_VULKAN, 1, 1);
    let ctx = with_provider(Some(&provider), || {
        assert!(!env(
            RETRO_ENVIRONMENT_SET_HW_RENDER,
            (&raw mut vulkan).cast()
        ));
    });
    assert!(ctx.hw_request.is_none());
    assert!(ctx.hw_refused.is_some_and(|r| r.contains("Vulkan")));

    let mut gl46 = request(RETRO_HW_CONTEXT_OPENGL_CORE, 4, 6);
    let ctx = with_provider(Some(&provider), || {
        assert!(!env(
            RETRO_ENVIRONMENT_SET_HW_RENDER,
            (&raw mut gl46).cast()
        ));
    });
    assert!(ctx.hw_refused.is_some());

    let mut gl33 = request(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 3);
    let ctx = with_provider(None, || {
        assert!(!env(
            RETRO_ENVIRONMENT_SET_HW_RENDER,
            (&raw mut gl33).cast()
        ));
    });
    assert!(ctx.hw_request.is_none() && ctx.hw_refused.is_some());
    assert!(gl33.get_current_framebuffer.is_none());
}

#[test]
fn preferred_hw_render_comes_from_the_provider() {
    let _serial = serial();
    let provider = FakeProvider::default();
    let mut kind: c_uint = 0;
    with_provider(Some(&provider), || {
        assert!(env(
            RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER,
            (&raw mut kind).cast()
        ));
    });
    assert_eq!(kind, RETRO_HW_CONTEXT_OPENGL_CORE);
    with_provider(None, || {
        assert!(!env(
            RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER,
            (&raw mut kind).cast()
        ));
    });
}

#[test]
fn host_fns_answer_zero_and_null_without_a_context() {
    let _serial = serial();
    with_provider(None, || {
        // SAFETY: plain host callbacks; the symbol is NUL-terminated.
        unsafe {
            assert_eq!(host_get_current_framebuffer(), 0);
            assert!(host_get_proc_address(c"glClear".as_ptr()).is_null());
        }
    });
}
