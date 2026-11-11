//! A game run from the data root to the end of the session (split from `main.rs`):
//! content preparation (archives, soft patches), the core's system files and
//! options (including a netplay room's multitap setup, resolved before the load for
//! every role), then `loaded_session_run`.

use std::cell::Cell;
use std::path::Path;

use anyhow::Context;
use system_catalog::Media;

use crate::loaded_session_run::{self, PreparedLaunch};
use crate::netplay_launch;
use crate::process_hard_exit::teardown_watchdog::TeardownWatchdog;
use crate::runner_cli_args::RunnerCliArgs;
use crate::runner_data_paths::RunnerDataPaths;
use crate::sdl_runtime::set_process_hints;
use crate::session_reporter::SessionReporter;
use crate::{archived_content, bios_option_switch, disc_bios_log, rom_soft_patching};
use crate::{runner_core_options, runner_session_config, staged_content_cleanup};

/// Secrets read from the environment at process start (RA login, netplay session).
pub type LaunchSecrets = (Option<achievements::Credentials>, Option<String>);

pub fn run(
    args: &RunnerCliArgs,
    (core, rom): (&Path, &Path),
    session_hooks: (&SessionReporter, &TeardownWatchdog, &Cell<bool>),
    (ra_login, netplay_env): LaunchSecrets,
) -> anyhow::Result<()> {
    // Before any SDL init: an OpenGL core initialises SDL video inside `load`.
    set_process_hints();
    let data_root = match &args.data_dir {
        Some(root) => root.clone(),
        None => ipc_protocol::data_root().context("cannot determine the user data directory")?,
    };
    let staging_dir =
        ipc_protocol::staged_content_dir(&data_root).join(std::process::id().to_string());
    staged_content_cleanup::remove_stale(
        &ipc_protocol::staged_content_dir(&data_root),
        &staging_dir,
        staged_content_cleanup::STALE_AGE,
    );
    let content =
        archived_content::PreparedContent::prepare(rom, args.system.as_deref(), &staging_dir)?;
    let system = content.system;
    let netplay = netplay_launch::prepare(args, system, netplay_env, session_hooks.0)?;
    let paths = RunnerDataPaths::resolve(
        Some(&data_root),
        rom,
        system,
        content
            .content_sha1
            .as_deref()
            .or(args.content_sha1.as_deref()),
    )?;
    // Disc images are never soft-patched nor preloaded (multi-file, hundreds of MB).
    let patched = match system.media {
        Media::Cartridge if content.path == rom => rom_soft_patching::load_soft_patched_rom(rom),
        Media::Cartridge => rom_soft_patching::load_soft_patched_rom_from(rom, &content.path),
        Media::Disc => rom_soft_patching::SoftPatchedRom::default(),
    };
    if system.media == Media::Disc {
        disc_bios_log::log_bios_choice(system, &paths.system_dir, rom);
    }
    runner_session_config::prepare_core(system, core, &paths.system_dir, rom)?;
    let bios = bios_option_switch::launch_option(system, &paths.system_dir, rom);
    let overrides = [bios.into_iter().collect(), args.core_options.clone()].concat();
    let players = netplay_launch::players(netplay.as_ref());
    let core_options = runner_core_options::resolve_for_players(system, players, &overrides);
    let launch = PreparedLaunch {
        content_rom: content.path.clone(),
        paths,
        patched,
        core_options,
        ra_login,
        netplay,
    };
    loaded_session_run::run(args, (core, rom), session_hooks, launch)
}
