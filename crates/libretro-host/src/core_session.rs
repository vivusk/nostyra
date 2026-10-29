//! `CoreSession`: one loaded core + game, driven frame by frame on one thread.
//!
//! Call order (libretro contract): `retro_set_environment` → `retro_init` →
//! `retro_set_video_refresh/audio_sample/audio_sample_batch/input_poll/input_state`
//! → `retro_load_game` → `retro_get_system_av_info`. `Drop` reverses it.

use std::ffi::CString;

use crate::av_callbacks;
use crate::core_library_loader::{CoreInfo, CoreLibrary};
use crate::core_session_hw_render::{self as hw, ProviderGuard};
use crate::environment_callback::environment;
use crate::ffi_types::*;
use crate::frame_sink_traits::{AudioSink, InputSource, PixelFormat, VideoSink};
use crate::host_context::{set_current, ActiveSinks, HostContext};
use crate::host_error::HostResult;
use crate::process_session_claim::ProcessSessionClaim;
pub use crate::session_config::SessionConfig;
use crate::session_config::{c_path, core_option_cstrings};
use crate::staged_content_file::StagedContentFile;

/// Timing + geometry reported by the core (`retro_system_av_info`).
pub type AvInfo = RetroSystemAvInfo;

/// A running core. `!Send`: all `retro_*` calls must stay on the creating thread
/// because callbacks find their context through a thread-local.
pub struct CoreSession {
    /// Owned (Box::into_raw); installed as this thread's callback context.
    pub(crate) ctx: *mut HostContext,
    pub(crate) initialized: bool,
    pub(crate) game_loaded: bool,
    /// Kept alive for the session: some cores reference the ROM buffer/path after load.
    pub(crate) rom_data: Vec<u8>,
    pub(crate) rom_path: CString,
    pub(crate) core_info: CoreInfo,
    /// `retro_serialize_size` once known (fixed-size cores only; see `serialization_quirks`).
    pub(crate) state_size: Option<usize>,
    /// Refuse states whose length differs from that size (see `set_exact_state_size`).
    pub(crate) exact_state_size: bool,
    // Field order matters: library unloads after Drop::drop ran, then the staged
    // content file is deleted (the core is gone), claim released last.
    pub(crate) core: CoreLibrary,
    /// Preloaded bytes written for a core that loads by path (see `staged_content_file`).
    pub(crate) staged: Option<StagedContentFile>,
    _claim: ProcessSessionClaim,
}

impl CoreSession {
    /// Loads the core and the game, following the libretro call order.
    pub fn load(config: SessionConfig<'_>) -> HostResult<Self> {
        let claim = ProcessSessionClaim::acquire()?;
        let core = CoreLibrary::open(config.core_path)?;
        Self::start(claim, core, config)
    }

    /// `load` after the library is open (tests start from a fake core).
    pub(crate) fn start(
        claim: ProcessSessionClaim,
        mut core: CoreLibrary,
        config: SessionConfig<'_>,
    ) -> HostResult<Self> {
        core.keep_loaded_if(config.keep_library_loaded); // also after a failed load
        crate::environment_core_messages::clear_last_core_error();
        // Fallible conversions first: nothing to clean up yet if they fail.
        let rom_path = c_path(config.rom_path)?;
        let mut host_ctx = HostContext::new(c_path(config.system_dir)?, c_path(config.save_dir)?);
        host_ctx.core_options = core_option_cstrings(config.core_options);
        let ctx = Box::into_raw(Box::new(host_ctx));
        set_current(ctx);
        // From here on, Drop performs cleanup on any early return.
        let mut session = Self {
            ctx,
            initialized: false,
            game_loaded: false,
            rom_data: Vec::new(),
            rom_path,
            core_info: CoreInfo::default(),
            state_size: None,
            exact_state_size: false,
            core,
            staged: None,
            _claim: claim,
        };
        // SAFETY: entry points resolved with libretro.h signatures; called in the
        // documented order on this thread with the context installed.
        unsafe {
            (session.core.set_environment)(environment);
            (session.core.init)();
            session.initialized = true;
            (session.core.set_video_refresh)(av_callbacks::video_refresh);
            (session.core.set_audio_sample)(av_callbacks::audio_sample);
            (session.core.set_audio_sample_batch)(av_callbacks::audio_sample_batch);
            (session.core.set_input_poll)(av_callbacks::input_poll);
            (session.core.set_input_state)(av_callbacks::input_state);
        }
        // Declared after `session`: dropped first, while the context still exists.
        let provider_guard = ProviderGuard::install(session.ctx, config.hw_render);
        session
            .load_game(config.rom_path, config.rom_data, config.staging_dir)
            .map_err(|error| hw::load_failure(session.ctx, error))?;
        let mut av = AvInfo::default();
        // SAFETY: valid out-param; game is loaded.
        unsafe { (session.core.get_system_av_info)(&mut av) };
        session.ctx_mut().av_info = av;
        session.ctx_mut().av_changed = false;
        hw::create_and_reset(session.ctx, av.geometry.max_width, av.geometry.max_height)?;
        drop(provider_guard);
        tracing::info!(
            fps = av.timing.fps,
            sample_rate = av.timing.sample_rate,
            "game loaded"
        );
        Ok(session)
    }

    pub(crate) fn ctx(&self) -> &HostContext {
        // SAFETY: owned pointer, valid until Drop; no callback runs while `&self` is used.
        unsafe { &*self.ctx }
    }

    pub(crate) fn ctx_mut(&mut self) -> &mut HostContext {
        // SAFETY: as `ctx`; never held across an FFI call that may re-enter callbacks.
        unsafe { &mut *self.ctx }
    }

    /// Emulates one frame; callbacks forward output into the given sinks.
    pub fn run_frame(
        &mut self,
        video: &mut (dyn VideoSink + 'static),
        audio: &mut (dyn AudioSink + 'static),
        input: &mut (dyn InputSource + 'static),
    ) -> HostResult<()> {
        self.ctx_mut().sinks = Some(ActiveSinks {
            video,
            audio,
            input,
        });
        self.ctx_mut().hw_frame = None; // only a frame presented by this run is read back
        hw::make_current(self.ctx);
        // SAFETY: game loaded; no Rust reference to the context is live across the call.
        unsafe { (self.core.run)() };
        let ctx = self.ctx_mut();
        ctx.sinks = None;
        ctx.take_frame_error()?;
        hw::deliver_frame(self.ctx, video);
        Ok(())
    }

    /// Current AV info (updated by `SET_SYSTEM_AV_INFO` / `SET_GEOMETRY`).
    pub fn av_info(&self) -> AvInfo {
        self.ctx().av_info
    }

    /// Returns the new AV info once after the core changed it, else `None`.
    pub fn take_av_change(&mut self) -> Option<AvInfo> {
        let ctx = self.ctx_mut();
        std::mem::take(&mut ctx.av_changed).then_some(ctx.av_info)
    }

    /// Core name/version as reported by `retro_get_system_info`.
    pub fn core_info(&self) -> &CoreInfo {
        &self.core_info
    }

    /// Pixel format the core negotiated via `SET_PIXEL_FORMAT`.
    pub fn pixel_format(&self) -> PixelFormat {
        self.ctx().pixel_format
    }
}

impl Drop for CoreSession {
    fn drop(&mut self) {
        // Graphics context current for the whole teardown; `context_destroy` first.
        let deinit_safe = hw::destroy(self.ctx);
        // SAFETY: mirrors the load sequence; only steps that succeeded are undone.
        unsafe {
            if self.game_loaded && deinit_safe {
                (self.core.unload_game)();
            }
            if self.initialized && deinit_safe {
                (self.core.deinit)();
            }
        }
        if self.initialized && !deinit_safe {
            tracing::warn!("core left loaded: it never got its graphics context");
            self.core.keep_loaded();
        }
        set_current(std::ptr::null_mut());
        // SAFETY: created by Box::into_raw in `load`; no longer reachable from callbacks.
        // Dropping it also drops the graphics context (before the library unloads).
        drop(unsafe { Box::from_raw(self.ctx) });
    }
}
