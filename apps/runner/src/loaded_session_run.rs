//! The part of a run from `CoreSession::load` to the end of the game (window,
//! headless or `--print-config`), split from `main.rs`. Owns the teardown guard: it is
//! declared right after the value owning the session on every path, so the
//! teardown watchdog arms just before the session (and the core) unloads:
//! after the load with `EXIT_RUNTIME` (early `?` returns), re-bound with the run's
//! own code after the window loop (declared after `app`) and inside
//! `headless_runner::run` (declared after its `session` parameter).

use std::cell::Cell;
use std::path::Path;

use anyhow::Context;
use libretro_host::{CoreSession, SessionConfig};
use system_catalog::Media;
use winit::event_loop::EventLoop;

use crate::achievement_session::{start_for_runner, SessionStart};
use crate::emulation_main_loop::EmulationApp;
use crate::game_save_data::GameSaveData;
use crate::headless_runner;
use crate::hw_render_setup::HwRenderSetup;
use crate::process_hard_exit::teardown_watchdog::{TeardownWatchdog, TestTeardownDelay};
use crate::rom_soft_patching::{self, SoftPatchedRom};
use crate::runner_cli_args::RunnerCliArgs;
use crate::runner_core_options;
use crate::runner_data_paths::RunnerDataPaths;
use crate::runner_effective_config::{self, EffectiveConfig};
use crate::runner_exit_codes::{exit_code_for, EXIT_RUNTIME};
use crate::runner_session_config;
use crate::sdl_runtime::SdlRuntime;
use crate::session_reporter::SessionReporter;

/// Everything `main` resolved before the core is loaded.
pub struct PreparedLaunch {
    pub paths: RunnerDataPaths,
    /// Soft patch of a cartridge (discs: none).
    pub patched: SoftPatchedRom,
    pub core_options: Vec<(String, String)>,
    /// RetroAchievements login from the launcher's env (cleared at process start).
    pub ra_login: Option<achievements::Credentials>,
}

/// Loads the core + game and plays it (see the module doc for the guard).
/// `core_kept_loaded` is set before a `keep_library_loaded` core is loaded: `main`
/// then ends the process with `exit_now` (no DLL detach), whatever the outcome.
pub fn run(
    args: &RunnerCliArgs,
    (core, rom): (&Path, &Path),
    (reporter, watchdog, core_kept_loaded): (&SessionReporter, &TeardownWatchdog, &Cell<bool>),
    launch: PreparedLaunch,
) -> anyhow::Result<()> {
    let PreparedLaunch {
        paths,
        patched,
        core_options,
        ra_login,
    } = launch;
    let system = paths.system;
    // Disc images are never preloaded (multi-file, hundreds of MB).
    let content = match system.media {
        Media::Cartridge => rom_soft_patching::content_bytes_for_core(rom, &patched),
        Media::Disc => None,
    };
    // Outlives `session` (declared first; moved into `EmulationApp` after it).
    let hw_render = HwRenderSetup::new();
    let keep_library_loaded = runner_core_options::keep_library_loaded(system);
    core_kept_loaded.set(keep_library_loaded); // even a failed load leaves it mapped
    let mut session = CoreSession::load(SessionConfig {
        core_path: core,
        rom_path: rom,
        system_dir: &paths.system_dir,
        save_dir: &paths.saves_dir,
        rom_data: content.as_deref(),
        staging_dir: Some(&paths.staging_dir),
        core_options: &core_options,
        hw_render: Some(hw_render.as_provider()),
        keep_library_loaded,
    })
    .with_context(|| format!("cannot start {} with {}", rom.display(), core.display()))?;
    let _slow_teardown = TestTeardownDelay; // debug test hook: drops after the guard armed
    let guard = watchdog.guard(EXIT_RUNTIME);
    runner_session_config::configure_session(&mut session, system)?;
    let config = EffectiveConfig::resolve(&paths, rom, &session.core_info().name, args);
    if args.print_config {
        let printed = runner_effective_config::print(config.resolved(), system.id);
        let _guard = guard.with_code(code_of(&printed));
        return printed;
    }

    // After the load, before the first frame; dropped before `session` on every path.
    let start = SessionStart {
        config: &config.resolved().achievements,
        env_credentials: ra_login,
        data_root: &paths.data_root,
        content_id: &paths.content_sha1,
        system,
        rom,
        patched: patched.data.clone(),
        core_stem: &core
            .file_stem()
            .map_or(String::new(), |s| s.to_string_lossy().into_owned()),
        core_version: &session.core_info().version,
        library_name: &session.core_info().name,
        core_options: &core_options,
        hardcore_ui: achievements::hardcore_ui_enabled(),
    };
    let achievements = start_for_runner(start, reporter);

    if let Some(frames) = args.headless_frames {
        reporter.started(&session.core_info().name, &paths.pending_sessions_dir);
        let run = (frames, achievements);
        return headless_runner::run(session, guard, run, args, &paths, &patched);
    }

    let saves =
        GameSaveData::load(&paths, &mut session, args.start_slot()).with_soft_patch(&patched);
    let (sdl, gamepads) = SdlRuntime::init().context("cannot initialise SDL")?;
    let event_loop = EventLoop::new().context("cannot create event loop")?;
    let mut app = EmulationApp::new(
        session, hw_render, saves, sdl, gamepads, args, reporter, &paths, config,
    );
    app.attach_achievements(achievements);
    let ran = event_loop.run_app(&mut app).context("event loop failed");
    let finished = app.finish(); // resume point + final SRAM flush (flush even if the loop errored)
    let result = ran.and(finished);
    // Declared after `app`: arms with the run's code before the session in it drops.
    let _guard = guard.with_code(code_of(&result));
    result
}

/// Exit code of a finished run: 0, or the error's (`exit_code_for`).
pub fn code_of(result: &anyhow::Result<()>) -> u8 {
    result.as_ref().map_or_else(exit_code_for, |()| 0)
}
