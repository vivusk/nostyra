//! Per-session state reachable from libretro's plain-C callbacks.
//!
//! libretro callbacks carry no user-data pointer, so the active session's
//! context lives behind a thread-local raw pointer on the emulation thread. It is
//! installed for the whole session (load → unload) because environment
//! callbacks also fire during `retro_set_environment` / `retro_load_game`.
//! No Mutex: callbacks run inside `retro_run` on the same thread, so a lock
//! would only add deadlock risk.

use std::cell::Cell;
use std::ffi::CString;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

use crate::environment_hw_render::HwRequestState;
use crate::ffi_disc_message_types::RetroDiskControlExtCallback;
use crate::ffi_types::RetroSystemAvInfo;
use crate::frame_sink_traits::{AudioSink, InputSource, PixelFormat, VideoSink};
use crate::hw_render::{HwRenderContext, HwRenderProvider};
use crate::memory_map_snapshot::MemoryMapSnapshot;
use crate::savestate_context::SavestateContext;
use crate::serialization_quirks::SerializationQuirks;

/// Which outputs the core should produce this frame (`GET_AUDIO_VIDEO_ENABLE`).
/// Disabled outputs are also discarded by the callbacks, whatever the core sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvEnable {
    pub video: bool,
    pub audio: bool,
}

impl AvEnable {
    /// Normal frame: picture and sound.
    pub const ALL: Self = Self {
        video: true,
        audio: true,
    };
    /// Skipped frame (fast-forward, run-ahead hidden frames): nothing rendered.
    pub const NONE: Self = Self {
        video: false,
        audio: false,
    };
}

impl Default for AvEnable {
    fn default() -> Self {
        Self::ALL
    }
}

/// Sinks borrowed for the duration of one `retro_run`.
///
/// `'static` bounds the sink *types* (they own their data); the references
/// themselves are short-lived and cleared right after `retro_run` returns.
pub(crate) struct ActiveSinks {
    pub video: *mut (dyn VideoSink + 'static),
    pub audio: *mut (dyn AudioSink + 'static),
    pub input: *mut (dyn InputSource + 'static),
}

/// Mutable state shared between `CoreSession` and the callbacks.
pub(crate) struct HostContext {
    pub pixel_format: PixelFormat,
    pub system_dir: CString,
    pub save_dir: CString,
    pub av_info: RetroSystemAvInfo,
    /// Set by `SET_SYSTEM_AV_INFO` / `SET_GEOMETRY`; consumed by the session owner.
    pub av_changed: bool,
    pub sinks: Option<ActiveSinks>,
    /// A callback body panicked (caught); surfaced as an error after `retro_run`.
    pub callback_panicked: bool,
    /// The core sent `RETRO_ENVIRONMENT_SHUTDOWN` (fatal emulator error).
    pub shutdown_requested: bool,
    /// Outputs enabled for the current frame (fast-forward skip, run-ahead).
    pub av_enable: AvEnable,
    /// Answer to `GET_SAVESTATE_CONTEXT`; non-`Normal` only during a run-ahead or
    /// rewind serialize/unserialize.
    pub savestate_context: SavestateContext,
    /// From `SET_SERIALIZATION_QUIRKS` (none if the core never says).
    pub serialization_quirks: SerializationQuirks,
    /// Host rule: re-query the state size on every serialize (`set_state_size_requery`).
    pub state_size_requery: bool,
    /// Host rule: no state call without a hardware context (`require_hw_context_for_states`).
    pub states_need_hw_context: bool,
    /// `GET_VARIABLE` answers (key, value); pointers handed out stay valid all session.
    pub core_options: Vec<(CString, CString)>,
    /// Disk control table registered by the core (multi-disc games).
    pub disk_control: Option<RetroDiskControlExtCallback>,
    /// On-screen core messages not yet taken by the frontend.
    pub core_messages: Vec<String>,
    /// Provider answering `SET_HW_RENDER`; set only while `CoreSession::load` runs
    /// (the `'static` is a lie told by `load`, see `core_session_hw_render`).
    pub hw_provider: Option<*const (dyn HwRenderProvider + 'static)>,
    /// What the core asked for in `SET_HW_RENDER` (accepted requests only).
    pub hw_request: Option<HwRequestState>,
    /// Why a `SET_HW_RENDER` was refused (reported if `retro_load_game` then fails).
    pub hw_refused: Option<String>,
    /// The session's graphics context: its single owner. Dropped with the context,
    /// after `retro_deinit` and before the core library unloads.
    pub hw_context: Option<Box<dyn HwRenderContext>>,
    /// `hw_context.framebuffer_id()`, cached for the `get_current_framebuffer` callback.
    pub hw_framebuffer_id: usize,
    /// Size of a frame the core finished in the framebuffer during this `retro_run`.
    pub hw_frame: Option<(u32, u32)>,
    /// `context_reset` was called: `context_destroy` is owed at teardown.
    pub hw_reset_done: bool,
    /// Latest `SET_MEMORY_MAPS` copy (`None`: the core never sent one).
    pub memory_maps: Option<MemoryMapSnapshot>,
}

impl HostContext {
    pub fn new(system_dir: CString, save_dir: CString) -> Self {
        Self {
            pixel_format: PixelFormat::Rgb1555,
            system_dir,
            save_dir,
            av_info: RetroSystemAvInfo::default(),
            av_changed: false,
            sinks: None,
            callback_panicked: false,
            shutdown_requested: false,
            av_enable: AvEnable::ALL,
            savestate_context: SavestateContext::Normal,
            serialization_quirks: SerializationQuirks::default(),
            state_size_requery: false,
            states_need_hw_context: false,
            core_options: Vec::new(),
            disk_control: None,
            core_messages: Vec::new(),
            hw_provider: None,
            hw_request: None,
            hw_refused: None,
            hw_context: None,
            hw_framebuffer_id: 0,
            hw_frame: None,
            hw_reset_done: false,
            memory_maps: None,
        }
    }
}

thread_local! {
    static CTX: Cell<*mut HostContext> = const { Cell::new(ptr::null_mut()) };
}

/// Installs `ctx` as this thread's active context (null to clear).
pub(crate) fn set_current(ctx: *mut HostContext) {
    CTX.with(|cell| cell.set(ctx));
}

/// Runs `f` with the active context from inside an `extern "C"` callback.
///
/// Returns `fallback` when no session is active or `f` panics; panics are
/// caught here because unwinding across the C frame is undefined behaviour.
pub(crate) fn with_current<R>(fallback: R, f: impl FnOnce(&mut HostContext) -> R) -> R {
    let ctx = CTX.with(Cell::get);
    if ctx.is_null() {
        return fallback;
    }
    // SAFETY: the pointer is installed by CoreSession for its lifetime and
    // cleared before the context is freed. Callbacks run on this thread while
    // the session is blocked inside an FFI call and holds no reference to it,
    // so this is the only live `&mut`.
    let ctx = unsafe { &mut *ctx };
    match catch_unwind(AssertUnwindSafe(|| f(&mut *ctx))) {
        Ok(value) => value,
        Err(_) => {
            ctx.callback_panicked = true;
            tracing::error!("panic inside libretro callback (caught)");
            fallback
        }
    }
}

/// Test helper: installs a fresh context, runs `f` (which may invoke callbacks)
/// and returns the context afterwards for inspection.
#[cfg(test)]
pub(crate) fn run_with_test_ctx(
    setup: impl FnOnce(&mut HostContext),
    f: impl FnOnce(),
) -> HostContext {
    let mut ctx = Box::new(HostContext::new(CString::default(), CString::default()));
    setup(&mut ctx);
    let raw = Box::into_raw(ctx);
    set_current(raw);
    f();
    set_current(ptr::null_mut());
    // SAFETY: `raw` came from Box::into_raw above and is no longer installed.
    *unsafe { Box::from_raw(raw) }
}
