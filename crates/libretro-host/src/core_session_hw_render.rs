//! Hardware-render lifecycle of a session, mirroring RetroArch's order:
//! `retro_load_game` (core sends `SET_HW_RENDER`) → `retro_get_system_av_info` →
//! create the context + framebuffer at the av maximum → make current →
//! `context_reset` → frames (make current before every core entry point) →
//! make current → `context_destroy` → `retro_unload_game` → `retro_deinit` → drop
//! the context (owned by `HostContext`, freed before the core library unloads).
//!
//! Functions take the raw context pointer because `context_reset`, `retro_run` and
//! `retro_serialize` re-enter `get_current_framebuffer` / `get_proc_address`, which
//! reach the context through `with_current`: no `&mut HostContext` may be alive
//! across those FFI calls, so every borrow below ends before the call.
//!
//! Provider lifetime: `SessionConfig::hw_render` only has to outlive `load`. Its
//! pointer is stored with a `'static` lifetime (a `*const dyn` cannot carry `'a`) and
//! cleared by [`ProviderGuard`] on every exit path of `load`.

use crate::core_session::CoreSession;
use crate::environment_hw_render::publish_session_framebuffer;
use crate::frame_sink_traits::VideoSink;
use crate::host_context::HostContext;
use crate::host_error::{HostError, HostResult};
use crate::hw_render::HwRenderProvider;

/// Installs the provider for `retro_load_game` and removes it when dropped.
pub(crate) struct ProviderGuard(*mut HostContext);

impl ProviderGuard {
    pub(crate) fn install(ctx: *mut HostContext, provider: Option<&dyn HwRenderProvider>) -> Self {
        let raw = provider.map(|p| {
            let short: *const (dyn HwRenderProvider + '_) = p;
            // SAFETY: only the trait-object lifetime changes; the pointer is cleared by
            // this guard's Drop before `load` returns, while the borrow is still live.
            unsafe {
                std::mem::transmute::<
                    *const (dyn HwRenderProvider + '_),
                    *const (dyn HwRenderProvider + 'static),
                >(short)
            }
        });
        // SAFETY: owned session context, valid until the session drops; no callback runs.
        unsafe { (*ctx).hw_provider = raw };
        Self(ctx)
    }
}

impl Drop for ProviderGuard {
    fn drop(&mut self) {
        // SAFETY: as in `install`; the session outlives the guard.
        unsafe { (*self.0).hw_provider = None };
    }
}

/// The reason to report when `retro_load_game` failed after a refused request.
pub(crate) fn load_failure(ctx: *mut HostContext, error: HostError) -> HostError {
    // SAFETY: owned session context; no callback runs.
    match unsafe { (*ctx).hw_refused.take() } {
        Some(reason) => HostError::HwRenderUnavailable(reason),
        None => error,
    }
}

/// After av info: builds the context if the core asked for one, then `context_reset`.
pub(crate) fn create_and_reset(
    ctx: *mut HostContext,
    max_width: u32,
    max_height: u32,
) -> HostResult<()> {
    let reset = {
        // SAFETY: owned session context; this borrow ends before any FFI call.
        let c = unsafe { &mut *ctx };
        let Some(state) = c.hw_request else {
            return Ok(());
        };
        // The `ProviderGuard` of `CoreSession::load` is still alive here.
        let provider = c.hw_provider.ok_or_else(|| {
            HostError::HwRenderUnavailable("no graphics provider for this session".into())
        })?;
        // SAFETY: the provider pointer is valid while `load` runs (guard still alive).
        let mut context = unsafe { &*provider }
            .create(&state.request, max_width, max_height)
            .map_err(HostError::HwRenderUnavailable)?;
        context
            .make_current()
            .map_err(HostError::HwRenderUnavailable)?;
        c.hw_framebuffer_id = context.framebuffer_id();
        publish_session_framebuffer(c.hw_framebuffer_id);
        c.hw_context = Some(context);
        state.context_reset
    };
    tracing::info!(max_width, max_height, "graphics context created");
    if let Some(reset) = reset {
        // SAFETY: core callback from SET_HW_RENDER; context current; no borrow live.
        unsafe { reset() };
    }
    // SAFETY: as above; the reset call has returned.
    unsafe { (*ctx).hw_reset_done = true };
    Ok(())
}

/// Makes the session's context current (no-op for software cores).
pub(crate) fn make_current(ctx: *mut HostContext) {
    // SAFETY: owned session context; the borrow ends before the caller's FFI call.
    let c = unsafe { &mut *ctx };
    if let Some(context) = c.hw_context.as_mut() {
        if let Err(error) = context.make_current() {
            tracing::error!(%error, "cannot make the graphics context current");
        }
    }
}

/// After `retro_run`: hands a finished HW frame to `video` (none if the core did not
/// present one this frame, or video was disabled).
pub(crate) fn deliver_frame(ctx: *mut HostContext, video: &mut dyn VideoSink) {
    // SAFETY: owned session context; `deliver_frame` makes no core call.
    let c = unsafe { &mut *ctx };
    let Some((width, height)) = c.hw_frame.take() else {
        return;
    };
    if let Some(context) = c.hw_context.as_mut() {
        context.deliver_frame(width, height, video);
    }
}

/// Teardown before `retro_unload_game`: make current, then `context_destroy` if (and
/// only if) `context_reset` ran. The context itself lives on until `HostContext` drops.
///
/// Returns whether `retro_unload_game` / `retro_deinit` may run: not for a core that
/// asked for a context but never got its `context_reset` (failed create, or a load
/// that failed after `SET_HW_RENDER`). GLideN64 (Mupen64Plus-Next) frees its GL
/// objects in `retro_deinit` through GL functions it never resolved and crashes;
/// PPSSPP's `retro_unload_game` finishes its boot and starts its renderer without a
/// context and crashes. Such a session only ends a failed load (the runner exits), so
/// the core is left loaded and initialised.
pub(crate) fn destroy(ctx: *mut HostContext) -> bool {
    make_current(ctx);
    let destroy = {
        // SAFETY: owned session context; the borrow ends before the FFI call.
        let c = unsafe { &mut *ctx };
        if !std::mem::take(&mut c.hw_reset_done) {
            return c.hw_request.is_none();
        }
        c.hw_request.and_then(|state| state.context_destroy)
    };
    if let Some(destroy) = destroy {
        // SAFETY: core callback from SET_HW_RENDER; context current; no borrow live.
        unsafe { destroy() };
    }
    publish_session_framebuffer(0); // withdraw the name; unload_game may still query it (gets 0)
    true
}

impl CoreSession {
    /// True when the core renders through a hardware context (e.g. OpenGL).
    pub fn uses_hw_context(&self) -> bool {
        self.ctx().hw_context.is_some()
    }

    /// Why the host refused the core's last `SET_HW_RENDER` request, when the core
    /// then loaded anyway (it went on in software, like PPSSPP).
    pub fn refused_hw_render(&self) -> Option<&str> {
        self.ctx().hw_refused.as_deref()
    }

    /// Makes the hardware context current before a core entry point (no-op otherwise).
    pub(crate) fn make_hw_current(&mut self) {
        make_current(self.ctx);
    }
}

#[cfg(test)]
#[path = "core_session_hw_render_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "core_session_fake_core_tests.rs"]
pub(crate) mod fake_core_tests;

#[cfg(test)]
#[path = "core_session_state_size_tests.rs"]
mod state_size_tests;
