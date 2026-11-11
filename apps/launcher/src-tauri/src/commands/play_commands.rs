//! Play: spawns the runner for a game (one session at a time) and hands it to the
//! session worker (`play_session_worker`), which follows it until it ends. Online
//! sessions (M20, `netplay_commands`) come through [`start_game`] too.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use ipc_protocol::StateSlotId;
use runner_session::{spawn_runner_session, DEFAULT_LAUNCH_TIMEOUT};
use tauri::{AppHandle, Manager, State};

use crate::app_state::{lock, ActiveSession, AppState, SessionControl};
use crate::cloud_unit_guard::{clear_units_for_play, session_unit_keys};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::achievements_mode::{hardcore_for_game, launch_load_slot};
use crate::commands::bios_required_check::require_bios;
use crate::commands::blocking::run_blocking;
use crate::commands::core_resolution::installed_core_for_platform;
use crate::commands::game_data_target::GameDataTarget;
use crate::commands::netplay_room_flow::NetplayPlay;
use crate::commands::play_launch_builder::build_runner_launch;
use crate::commands::play_session_worker::{spawn_session_worker, SessionNetplay};
use crate::commands::save_state_commands::load_state_args;
use crate::gamepad_capture_state::GamepadCapture;

/// Launches `game_id`, optionally loading save-state slot `load_state` (`0..=9` or
/// `"auto"` = Continue from the resume point) at start (runner `--load-state N|auto`;
/// an empty/incompatible slot only shows an error in-game, a failed resume boots
/// fresh; never a state in achievement hardcore). The ROM path comes from the
/// database, never from the UI. Runs off the main thread (cloud apply-pass wait).
#[tauri::command]
pub async fn play_game(
    app: AppHandle,
    game_id: i64,
    load_state: Option<StateSlotId>,
) -> CommandResult<()> {
    run_blocking(move || start_game(&app, game_id, load_state, None)).await
}

/// Checks, claims the session slot and spawns the runner; `netplay` adds the online
/// flags + env (always softcore: the runner drops to softcore too).
pub(crate) fn start_game(
    app: &AppHandle,
    game_id: i64,
    load_state: Option<StateSlotId>,
    netplay: Option<NetplayPlay>,
) -> CommandResult<()> {
    let state = app.state::<AppState>();
    let game = storage_db::get_game(&state.db(), game_id)?
        .ok_or_else(|| CommandError::invalid("This game is no longer in the library."))?;
    let target = GameDataTarget::from_game(&game);
    let hardcore = netplay.is_none() && hardcore_for_game(&state.paths.data_root, &target);
    let load_args = load_state_args(launch_load_slot(load_state, hardcore))?;
    if game.missing {
        return Err(CommandError::invalid(
            "The ROM file was not found. Rescan your library.",
        ));
    }
    let (spec, core) = installed_core_for_platform(&state.paths.cores_dir, &game.platform_id)?;
    // A core without an HLE BIOS (PS2) needs a valid dump: typed error → BIOS picker.
    require_bios(
        &ipc_protocol::system_dir(&state.paths.data_root),
        spec,
        &game.platform_id,
    )?;
    // The runner must own the pads alone: stop any capture (waits until SDL is
    // dropped) and refuse new ones until the session is registered (then the
    // capture's own session check refuses). Taken before the session lock: a
    // capture checks the session while holding the capture lock.
    let capture = app.state::<GamepadCapture>();
    let _no_capture = capture.block_for_play()?;
    // Lock order session → core_download (same as download_core) and session → cloud
    // job (`cloud_sync_state`): checking and claiming happen under the session lock, so
    // Play never overlaps a core update or a cloud sync of the game's units.
    let mut session_slot = lock(&state.session);
    if session_slot.is_some() {
        return Err(CommandError::new("session", "A game is already running."));
    }
    if lock(&state.core_download).is_some() {
        return Err(CommandError::new(
            "core",
            "Wait for the core download to finish.",
        ));
    }
    // A cloud job on this game's units: a transfer is cancelled, an apply pass
    // (renames, ms-scale) is waited for. Never refused because of an upload.
    let unit_keys = session_unit_keys(&target);
    clear_units_for_play(&state.cloud, &unit_keys);
    let resources = app.path().resource_dir().ok();
    let launch = build_runner_launch(
        &state.paths,
        resources.as_deref(),
        game,
        core,
        load_args,
        netplay.as_ref(),
    );
    let session = spawn_runner_session(&launch, DEFAULT_LAUNCH_TIMEOUT).map_err(|err| {
        CommandError::new("session", format!("Cannot start the game runner: {err}"))
    })?;
    let control = SessionControl {
        killer: session.killer.clone(),
        cancelled: Arc::default(),
    };
    *session_slot = Some(ActiveSession {
        game_id,
        unit_keys,
        control: Some(control),
    });
    drop(session_slot);
    tracing::info!(
        game_id,
        pid = session.pid,
        online = netplay.is_some(),
        "game session started"
    );
    let netplay = netplay.map(|play| SessionNetplay {
        role: play.role,
        start: play.start,
    });
    spawn_session_worker(app.clone(), session, game_id, netplay);
    Ok(())
}

/// Running game id, if any (UI state after a reload).
#[tauri::command]
pub fn get_active_session(state: State<'_, AppState>) -> Option<i64> {
    lock(&state.session).as_ref().map(|s| s.game_id)
}

/// Online lobby Cancel: ends the runner (it has no command channel; nothing was
/// played yet). The session then ends as usual, without an error toast.
#[tauri::command]
pub fn quit_session(state: State<'_, AppState>) {
    let session = lock(&state.session);
    if let Some(control) = session.as_ref().and_then(|s| s.control.as_ref()) {
        control.cancelled.store(true, Ordering::SeqCst);
        control.killer.kill();
        tracing::info!("game session cancelled");
    }
}
