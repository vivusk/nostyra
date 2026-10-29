//! `SET_HW_RENDER` / `GET_PREFERRED_HW_RENDER`, plus the two host functions handed to
//! the core in `retro_hw_render_callback` (`get_current_framebuffer`,
//! `get_proc_address`). The decision is the provider's (`HwRenderProvider::supports`);
//! no provider → every request is refused (software-only session).

use std::ffi::{c_char, c_uint, c_void, CStr};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ffi_hw_render_types::*;
use crate::host_context::{with_current, HostContext};
use crate::hw_render::{HwContextKind, HwRenderRequest};

/// An accepted `SET_HW_RENDER`: the request plus the core's lifecycle callbacks.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HwRequestState {
    pub request: HwRenderRequest,
    pub context_reset: Option<RetroHwContextResetFn>,
    pub context_destroy: Option<RetroHwContextResetFn>,
}

/// # Safety
/// `data` must be null or point to the type libretro.h documents for `cmd`.
pub(crate) unsafe fn hw_render(ctx: &mut HostContext, cmd: c_uint, data: *mut c_void) -> bool {
    if data.is_null() {
        return false;
    }
    match cmd {
        RETRO_ENVIRONMENT_SET_HW_RENDER => {
            // SAFETY: `struct retro_hw_render_callback *` (in/out).
            let callback = unsafe { &mut *data.cast::<RetroHwRenderCallback>() };
            set_hw_render(ctx, callback)
        }
        RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER => {
            let Some(provider) = ctx.hw_provider else {
                return false;
            };
            // SAFETY: valid while `CoreSession::load` runs (cleared on every exit path).
            let kind = unsafe { &*provider }.preferred();
            // SAFETY: `unsigned *` out-param (`enum retro_hw_context_type`).
            unsafe { data.cast::<c_uint>().write(context_type(kind)) };
            true
        }
        _ => false,
    }
}

fn set_hw_render(ctx: &mut HostContext, callback: &mut RetroHwRenderCallback) -> bool {
    let request = request_from(callback);
    let supported = match ctx.hw_provider {
        // SAFETY: valid while `CoreSession::load` runs (cleared on every exit path).
        Some(provider) => unsafe { &*provider }.supports(&request),
        None => false,
    };
    if !supported {
        let reason = format!("no graphics backend for {:?}", request.kind);
        tracing::warn!(?request, "SET_HW_RENDER refused: {reason}");
        ctx.hw_refused = Some(reason);
        return false;
    }
    tracing::info!(?request, "SET_HW_RENDER accepted");
    callback.get_current_framebuffer = Some(host_get_current_framebuffer);
    callback.get_proc_address = Some(host_get_proc_address);
    ctx.hw_request = Some(HwRequestState {
        request,
        context_reset: callback.context_reset,
        context_destroy: callback.context_destroy,
    });
    ctx.hw_refused = None;
    true
}

fn request_from(callback: &RetroHwRenderCallback) -> HwRenderRequest {
    let kind = match callback.context_type {
        RETRO_HW_CONTEXT_OPENGL => HwContextKind::OpenGl,
        RETRO_HW_CONTEXT_OPENGL_CORE => HwContextKind::OpenGlCore {
            major: callback.version_major,
            minor: callback.version_minor,
        },
        RETRO_HW_CONTEXT_OPENGLES2 => HwContextKind::OpenGlEs2,
        RETRO_HW_CONTEXT_OPENGLES3 => HwContextKind::OpenGlEs3,
        RETRO_HW_CONTEXT_VULKAN => HwContextKind::Vulkan,
        other => HwContextKind::Other(other),
    };
    HwRenderRequest {
        kind,
        depth: callback.depth,
        stencil: callback.stencil,
        bottom_left_origin: callback.bottom_left_origin,
        cache_context: callback.cache_context,
        debug: callback.debug_context,
    }
}

fn context_type(kind: HwContextKind) -> c_uint {
    match kind {
        HwContextKind::OpenGl => RETRO_HW_CONTEXT_OPENGL,
        HwContextKind::OpenGlCore { .. } => RETRO_HW_CONTEXT_OPENGL_CORE,
        HwContextKind::OpenGlEs2 => RETRO_HW_CONTEXT_OPENGLES2,
        HwContextKind::OpenGlEs3 => RETRO_HW_CONTEXT_OPENGLES3,
        HwContextKind::Vulkan => RETRO_HW_CONTEXT_VULKAN,
        HwContextKind::Other(raw) => raw,
    }
}

/// `get_current_framebuffer`: the session's stable framebuffer (0 before it exists).
/// Re-entered from `context_reset` / `retro_run`; reads the context through
/// `with_current`, so the session must hold no `&mut HostContext` across those calls.
unsafe extern "C" fn host_get_current_framebuffer() -> usize {
    with_current(None, |ctx| Some(ctx.hw_framebuffer_id))
        .unwrap_or_else(|| SESSION_FRAMEBUFFER.load(Ordering::Acquire))
}

/// The session's framebuffer name for `get_current_framebuffer` calls made off the
/// session thread, where the thread-local context is absent: PPSSPP's emulation
/// thread asks for it every frame (`SetRenderTarget`), and a 0 there would send its
/// output to the hidden window instead of our framebuffer. One session per process
/// (`ProcessSessionClaim`), so one value; 0 while no context exists.
static SESSION_FRAMEBUFFER: AtomicUsize = AtomicUsize::new(0);

/// Publishes (or, with 0, withdraws) the framebuffer name for other threads.
pub(crate) fn publish_session_framebuffer(id: usize) {
    SESSION_FRAMEBUFFER.store(id, Ordering::Release);
}

/// `get_proc_address`: resolved by the session's context; null before it exists.
unsafe extern "C" fn host_get_proc_address(sym: *const c_char) -> *const c_void {
    if sym.is_null() {
        return std::ptr::null();
    }
    // SAFETY: the core passes a NUL-terminated symbol name valid for this call.
    let name = unsafe { CStr::from_ptr(sym) };
    with_current(std::ptr::null(), |ctx| match ctx.hw_context.as_ref() {
        Some(context) => context.proc_address(name),
        None => std::ptr::null(),
    })
}

#[cfg(test)]
#[path = "environment_hw_render_tests.rs"]
mod tests;
