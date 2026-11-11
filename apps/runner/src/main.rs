//! `nostyra-runner`: plays one ROM with one libretro core in its own process.
//!
//! A `.bps`/`.ups`/`.ips` named like the ROM is soft-patched in (`rom_soft_patching`).
//! `nostyra-runner --core <lib> --rom <file> [--fullscreen] [--audio-sync]
//! [--state-slot N | --load-state N|auto] [--data-dir DIR] [--ipc] [--print-config]` or
//! `nostyra-runner --probe-core <lib>`. Gameplay settings come from the config
//! layers under `<data>/config/` (`runner_effective_config`).
//! Exit codes: see `runner_exit_codes`; launcher events: see `session_reporter`.

#![deny(unsafe_code)]

mod achievement_overlay;
mod achievement_session;
#[cfg(test)]
mod achievements_disc_format_tests;
#[cfg(test)]
mod achievements_fake_set_session;
#[cfg(test)]
mod achievements_progress_real_core_tests;
#[cfg(test)]
mod achievements_real_core_checks;
#[cfg(test)]
mod achievements_real_core_loader;
#[cfg(test)]
mod achievements_real_core_suite_tests;
mod archived_content;
mod auto_save_on_exit;
mod bios_option_switch;
mod core_probe_command;
mod core_system_assets;
mod disc_bios_log;
#[cfg(test)]
mod dreamcast_real_core_checks_tests;
#[cfg(test)]
mod dreamcast_real_core_suite_tests;
mod emulation_main_loop;
mod frame_pacer;
mod game_launch;
mod game_save_data;
#[cfg(test)]
mod handheld_real_core_suite_tests;
mod hardcore_gate;
mod headless_runner;
mod hw_render_setup;
#[cfg(test)]
mod hw_render_setup_core_tests;
mod ipc_event_writer;
mod loaded_session_run;
mod loop_timing;
#[cfg(test)]
mod n64_real_core_suite_tests;
mod netplay_gate;
mod netplay_launch;
mod netplay_overlay;
mod netplay_runtime;
#[cfg(feature = "netplay-spike")]
mod netplay_spike;
#[cfg(test)]
mod netplay_synctest_core_tests;
mod on_screen_status;
mod pause_menu;
mod periodic_resume_save;
mod pre_boot_load_guard;
mod process_hard_exit;
#[cfg(test)]
mod psp_real_core_suite_tests;
#[cfg(test)]
mod psx_real_core_suite_tests;
#[cfg(test)]
mod ra_memory_regions_real_core_tests;
#[cfg(test)]
mod real_core_session_loader;
#[cfg(test)]
mod reset_core_tests;
#[cfg(test)]
mod rewind_core_tests;
#[cfg(test)]
mod rewind_hardcore_core_tests;
mod rewind_ring;
mod rom_soft_patching;
#[cfg(test)]
mod run_ahead_core_tests;
mod runner_cli_args;
mod runner_core_options;
mod runner_data_paths;
mod runner_effective_config;
mod runner_exit_codes;
mod runner_held_input;
mod runner_log_file;
#[cfg(test)]
mod runner_real_core_suite;
mod runner_session_config;
mod runner_window;
mod runner_window_icon;
mod save_state_slots;
#[cfg(test)]
mod save_state_slots_core_tests;
mod sdl_runtime;
#[cfg(test)]
mod sega_real_core_suite_tests;
mod session_reporter;
mod shader_resource_dir;
mod sram_persistence;
mod staged_content_cleanup;
mod startup_boot_gate;
mod startup_state_load;
mod state_file_guard;
mod state_load_checks;
#[cfg(test)]
mod state_policy_core_tests;
mod state_thumbnail_writer;

use std::cell::Cell;
use std::process::ExitCode;

use clap::Parser;

use crate::loaded_session_run::code_of;
use crate::process_hard_exit::teardown_watchdog::TeardownWatchdog;
use crate::runner_cli_args::RunnerCliArgs;
use crate::runner_exit_codes::EXIT_GRAPHICS;
use crate::session_reporter::SessionReporter;

fn main() -> ExitCode {
    // Before any other thread exists: the token must not linger in our environment.
    let ra_login = achievement_session::achievement_credentials::take_env_credentials();
    let netplay_env = netplay_launch::take_session_env(); // same rule: before any thread
    #[cfg(feature = "netplay-spike")]
    if let Some(code) = netplay_spike::run_from_args() {
        return code;
    }
    let args = RunnerCliArgs::parse(); // exits with code 2 on bad arguments
                                       // Logs go to stderr (stdout is reserved for machine-readable output) and a log file.
    runner_log_file::start_runner_logging(&args);
    if let Some(core) = &args.probe_core {
        return core_probe_command::run(core);
    }
    // clap guarantees both without --probe-core (required_unless_present).
    let (Some(core), Some(rom)) = (&args.core, &args.rom) else {
        return ExitCode::from(2);
    };
    let reporter = SessionReporter::new(args.ipc, rom);
    let watchdog = TeardownWatchdog::reporting(&reporter);
    let core_kept_loaded = Cell::new(false); // set before a kept-loaded core is loaded
    let hooks = (&reporter, &watchdog, &core_kept_loaded);
    let secrets = (ra_login, netplay_env);
    let result = game_launch::run(&args, (core, rom), hooks, secrets);
    let code = code_of(&result);
    let error = result.err().map(|err| {
        tracing::error!("{err:#}");
        format!("{err:#}")
    });
    watchdog.disarm(); // the session is gone: the teardown finished in time
    reporter.finished(code, error);
    // A kept-loaded core (Dolphin) never sees a normal exit: its DLL detach / static
    // destructors can block `ExitProcess` forever (seen after a failed boot, ~1 in 15),
    // and a core kept loaded after a refused GL context would crash it.
    if code == EXIT_GRAPHICS || core_kept_loaded.get() {
        process_hard_exit::exit_now(code);
    }
    ExitCode::from(code)
}
