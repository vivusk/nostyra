//! Assembles the [`RunnerLaunch`] of a Play: paths from the database and registry,
//! `--content-sha1`, the bundled shaders, the state to load, the RA login env and,
//! for an online session (M20), the `--netplay-*` flags plus the
//! `NOSTYRA_NETPLAY_SESSION` env (secrets in env only, never argv).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use runner_session::RunnerLaunch;
use storage_db::Game;

use crate::app_state::AppPaths;
use crate::commands::achievements_commands::runner_login_env;
use crate::commands::netplay_room_flow::NetplayPlay;
use crate::shader_presets_dir::runner_shader_args;

/// The runner invocation for `game` on `core` (`load_args` = `--load-state …`).
pub fn build_runner_launch(
    paths: &AppPaths,
    resource_dir: Option<&Path>,
    game: Game,
    core: PathBuf,
    load_args: Vec<OsString>,
    netplay: Option<&NetplayPlay>,
) -> RunnerLaunch {
    let netplay_args = netplay.map(|play| play.args.clone()).unwrap_or_default();
    // RA login via env only (argv is visible in process lists).
    let mut env = runner_login_env(&paths.data_root);
    env.extend(netplay.map(NetplayPlay::env));
    RunnerLaunch {
        runner: paths.runner.clone(),
        core,
        rom: game.rom_path.into(),
        system: game.platform_id,
        data_dir: paths.data_root.clone(),
        extra_args: [
            content_sha1_args(&game.sha1),
            runner_shader_args(resource_dir),
            load_args,
            netplay_args,
        ]
        .concat(),
        env,
    }
}

/// `--content-sha1 <sha1>` (save folder id, spares the runner hashing a disc); none
/// for a stored sha1 the runner would reject, which then hashes the ROM itself.
fn content_sha1_args(sha1: &str) -> Vec<OsString> {
    match ipc_protocol::content_id(sha1) {
        Some(_) => vec!["--content-sha1".into(), sha1.into()],
        None => Vec::new(),
    }
}
