//! Runner loop (winit `ApplicationHandler`): emulation runs on the main thread.
//! One redraw = pump SDL events → pause menu input (no frames while it is open) →
//! emulate due frames (paced by `LoopTiming`, or uncapped while fast-forward is held,
//! or stepped back while rewind is held) → present. Keyboard comes from winit,
//! gamepads from SDL.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use config_layers::ResolvedConfig;
use input_mapping::{AnalogInputs, InputBindings, JoypadInputSource, SdlGamepadManager};
use libretro_host::CoreSession;
use sdl3::GamepadSubsystem;
use system_catalog::SaveStateQuality;
use video_renderer::{GpuFramePresenter, RgbaFrameBuffer};
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use self::core_message_notices::CoreNotices;
use self::emulate_time_stats::EmulateTimeStats;
use self::pointer_events::MousePointer;
use self::rewind_playback::Rewind;
use self::run_ahead::RunAhead;
use crate::achievement_session::AchievementSession;
use crate::game_save_data::GameSaveData;
use crate::hw_render_setup::HwRenderSetup;
use crate::loop_timing::LoopTiming;
use crate::on_screen_status::OnScreenStatus;
use crate::pause_menu::PauseMenu;
use crate::runner_cli_args::RunnerCliArgs;
use crate::runner_core_options;
use crate::runner_data_paths::RunnerDataPaths;
use crate::runner_effective_config::EffectiveConfig;
use crate::sdl_runtime::SdlRuntime;
use crate::session_reporter::SessionReporter;
use crate::startup_state_load::StartupStateLoad;

mod achievement_glue;
mod core_message_notices;
mod display_sync;
mod emulate_time_stats;
mod fast_forward;
mod hotkey_actions;
mod pause_glue;
pub(crate) mod pointer_events;
mod present_step;
pub(crate) mod rewind_playback;
pub(crate) mod run_ahead;
pub(crate) mod save_state_policy;
mod session_exit;
mod shader_glue;
mod window_events;
mod window_resize_on_geometry;

pub struct EmulationApp<'a> {
    achievements: Option<AchievementSession>, // RA session; first: drops before `session`
    session: CoreSession,
    /// Graphics context; drops after `session` (core GL teardown while SDL video lives).
    _hw_render: HwRenderSetup,
    /// The core's save-state quality (run-ahead / rewind policy, menu rebuilds).
    savestates: SaveStateQuality,
    frame: RgbaFrameBuffer,
    input: JoypadInputSource,
    /// Turbo A phase: bumped once per real-timeline frame (shared with `input`).
    turbo_clock: Rc<Cell<u64>>,
    /// Mouse → touch screen (`SystemSpec.touch_screen` systems only).
    pointer: Option<MousePointer>,
    sdl: SdlRuntime,
    timing: LoopTiming,
    window: Option<Arc<Window>>,
    presenter: Option<GpuFramePresenter>,
    display_hz: Option<f64>,
    saves: GameSaveData,
    start_fullscreen: bool,
    /// Short in-picture messages (state hotkeys), bottom-left for a few seconds.
    status: OnScreenStatus,
    /// Rate limit for core messages shown in `status`.
    core_notices: CoreNotices,
    /// `--shaders-dir` (first place to look for bundled presets).
    shaders_dir: Option<PathBuf>,
    /// Fast-forward was held on the last redraw (title + pacer reset on change).
    fast_forwarding: bool,
    /// Told when the game window is up (`Started` for the launcher).
    reporter: &'a SessionReporter,
    pending_sessions_dir: PathBuf,
    /// First fatal error; `main` reports it after the event loop exits.
    error: Option<anyhow::Error>,
    /// Live config: layers + CLI, then pause-menu changes (saved only via `config_layers`).
    config: ResolvedConfig,
    /// Layer files behind `config` (menu "Save for this game / as default / reset").
    config_layers: EffectiveConfig,
    /// In-game pause menu (game paused while open).
    menu: PauseMenu,
    /// `latency.run_ahead_frames` (0 = plain frames).
    run_ahead: RunAhead,
    /// Hold-to-rewind history (`rewind.enabled`; no memory used when off).
    rewind: Rewind,
    emulate_stats: EmulateTimeStats,
    /// `--load-state N|auto` until it has loaded or failed (tried before each redraw's frames).
    startup_load: Option<StartupStateLoad>,
}

impl<'a> EmulationApp<'a> {
    #[allow(clippy::too_many_arguments)] // one call site (main); a struct would only rename the same fields
    pub fn new(
        session: CoreSession,
        hw_render: HwRenderSetup,
        saves: GameSaveData,
        sdl: SdlRuntime,
        gamepads: GamepadSubsystem,
        args: &RunnerCliArgs,
        reporter: &'a SessionReporter,
        paths: &RunnerDataPaths,
        config_layers: EffectiveConfig,
    ) -> Self {
        let timing = LoopTiming::new(sdl.audio(), session.av_info(), args.audio_sync);
        let config = config_layers.resolved().clone();
        let savestates = runner_core_options::savestate_quality(paths.system);
        let run_ahead = RunAhead::new(config.latency.run_ahead_frames, &session, savestates);
        let rewind = Rewind::new(&config.rewind, &session, savestates);
        // Bindings are read once: config changes apply on the next launch.
        let system_actions = Some(config_layers.system_actions());
        let (bindings, warnings) = InputBindings::from_config(&config.input, system_actions);
        for warning in warnings {
            tracing::warn!("input bindings: {warning}");
        }
        let analog = AnalogInputs {
            sticks: paths.system.analog_sticks,
            triggers: paths.system.analog_triggers,
        };
        let gamepads = SdlGamepadManager::new(gamepads, bindings, analog);
        let mut input = JoypadInputSource::new(gamepads);
        let turbo_clock = input.turbo_a_mut().frame_clock();
        let pointer = MousePointer::attach(paths.system.touch_screen, &mut input);
        Self {
            achievements: None,
            session,
            _hw_render: hw_render,
            savestates,
            frame: RgbaFrameBuffer::default(),
            input,
            turbo_clock,
            pointer,
            sdl,
            timing,
            window: None,
            presenter: None,
            display_hz: None,
            saves,
            start_fullscreen: args.fullscreen,
            fast_forwarding: false,
            status: OnScreenStatus::default(),
            core_notices: CoreNotices::default(),
            shaders_dir: args.shaders_dir.clone(),
            reporter,
            pending_sessions_dir: paths.pending_sessions_dir.clone(),
            error: None,
            config,
            config_layers,
            menu: PauseMenu::default(),
            run_ahead,
            rewind,
            emulate_stats: EmulateTimeStats::new(),
            startup_load: args
                .load_state
                .map(|slot| StartupStateLoad::new(slot, savestates)),
        }
    }

    fn report_started(&self) {
        self.reporter
            .started(&self.session.core_info().name, &self.pending_sessions_dir);
    }

    /// One redraw: input → emulate due frames (none while paused) → present.
    fn tick(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        if self.sdl.pump(&mut self.input) {
            event_loop.exit();
            return Ok(());
        }
        if self.update_pause_menu(event_loop) {
            return self.present(); // last frame under the menu
        }
        self.update_pointer();
        self.apply_startup_load();
        self.emulate_due_frames()?;
        self.show_core_messages();
        if let Some(av) = self.session.take_av_change() {
            let prefs = self
                .timing
                .on_av_change(av, self.sdl.audio(), self.display_hz);
            self.apply_present_mode(prefs);
            self.apply_geometry(av.geometry); // picture aspect; window on a layout change
        }
        self.present()
    }
}
