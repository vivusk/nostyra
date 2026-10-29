//! Play: spawns the runner for a game (one session at a time) and follows it on a
//! worker thread until it ends, then records play time and notifies the UI.

use std::ffi::OsString;

use ipc_protocol::StateSlotId;
use runner_session::{
    import_pending_sessions, spawn_runner_session, PendingImportSummary, RunnerLaunch,
    SessionOutcome, SessionUpdate, DEFAULT_LAUNCH_TIMEOUT,
};
use storage_db::NewPlaySession;
use tauri::{AppHandle, Manager, State};

use crate::app_events::{
    emit, SessionEndedPayload, SessionErrorPayload, SessionPayload, SESSION_ENDED,
    SESSION_LAUNCH_TIMEOUT, SESSION_STARTED,
};
use crate::app_state::{lock, ActiveSession, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::achievements_commands::{on_achievement_update, runner_login_env};
use crate::commands::achievements_mode::{hardcore_for_game, launch_load_slot};
use crate::commands::core_resolution::installed_core_for_platform;
use crate::commands::game_data_target::GameDataTarget;
use crate::commands::save_state_commands::load_state_args;
use crate::gamepad_capture_state::GamepadCapture;
use crate::shader_presets_dir::runner_shader_args;

/// Launches `game_id`, optionally loading save-state slot `load_state` (`0..=9` or
/// `"auto"` = Continue from the resume point) at start (runner `--load-state N|auto`;
/// an empty/incompatible slot only shows an error in-game, a failed resume boots
/// fresh; never a state in achievement hardcore). The ROM path comes from the
/// database, never from the UI.
#[tauri::command]
pub fn play_game(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: i64,
    load_state: Option<StateSlotId>,
) -> CommandResult<()> {
    let game = storage_db::get_game(&state.db(), game_id)?
        .ok_or_else(|| CommandError::invalid("This game is no longer in the library."))?;
    let target = GameDataTarget::from_game(&game);
    let hardcore = hardcore_for_game(&state.paths.data_root, &target);
    let load_args = load_state_args(launch_load_slot(load_state, hardcore))?;
    if game.missing {
        return Err(CommandError::invalid(
            "The ROM file was not found. Rescan your library.",
        ));
    }
    let (_, core) = installed_core_for_platform(&state.paths.cores_dir, &game.platform_id)?;
    // The runner must own the pads alone: stop any capture (waits until SDL is
    // dropped) and refuse new ones until the session is registered (then the
    // capture's own session check refuses). Taken before the session lock: a
    // capture checks the session while holding the capture lock.
    let capture = app.state::<GamepadCapture>();
    let _no_capture = capture.block_for_play()?;
    // Lock order session → core_download (same as download_core): checking and claiming
    // happen under both locks, so Play and a core update can never overlap.
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
    let launch = RunnerLaunch {
        runner: state.paths.runner.clone(),
        core,
        rom: game.rom_path.into(),
        system: game.platform_id,
        data_dir: state.paths.data_root.clone(),
        extra_args: [
            content_sha1_args(&game.sha1),
            runner_shader_args(app.path().resource_dir().ok().as_deref()),
            load_args,
        ]
        .concat(),
        // RA login via env only (argv is visible in process lists).
        env: runner_login_env(&state.paths.data_root),
    };
    let session = spawn_runner_session(&launch, DEFAULT_LAUNCH_TIMEOUT).map_err(|err| {
        CommandError::new("session", format!("Cannot start the game runner: {err}"))
    })?;
    *session_slot = Some(ActiveSession { game_id });
    drop(session_slot);
    tracing::info!(game_id, pid = session.pid, "game session started");

    // spawn_blocking cannot fail, so the claimed slot is always released by finish_session.
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        for update in session.updates {
            match update {
                SessionUpdate::Started { .. } => {
                    emit(&worker_app, SESSION_STARTED, SessionPayload { game_id })
                }
                SessionUpdate::LaunchTimedOut => emit(
                    &worker_app,
                    SESSION_LAUNCH_TIMEOUT,
                    SessionPayload { game_id },
                ),
                SessionUpdate::Achievements(update) => {
                    on_achievement_update(&worker_app, game_id, &update)
                }
                SessionUpdate::Finished(outcome) => finish_session(&worker_app, game_id, &outcome),
            }
        }
    });
    Ok(())
}

/// `--content-sha1 <sha1>` (save folder id, spares the runner hashing a disc); none
/// for a stored sha1 the runner would reject, which then hashes the ROM itself.
fn content_sha1_args(sha1: &str) -> Vec<OsString> {
    match ipc_protocol::content_id(sha1) {
        Some(_) => vec!["--content-sha1".into(), sha1.into()],
        None => Vec::new(),
    }
}

/// Running game id, if any (UI state after a reload).
#[tauri::command]
pub fn get_active_session(state: State<'_, AppState>) -> Option<i64> {
    lock(&state.session).map(|s| s.game_id)
}

/// Records play time (runner's pending file first, launcher measurement as a
/// fallback when the runner crashed before writing it) and frees the session slot.
fn finish_session(app: &AppHandle, game_id: i64, outcome: &SessionOutcome) {
    let state = app.state::<AppState>();
    {
        let conn = state.db();
        let imported =
            import_pending_sessions(&conn, &state.paths.data_root).unwrap_or_else(|err| {
                tracing::warn!(%err, "pending session import failed");
                PendingImportSummary::default()
            });
        if let Some(measured) = measured_fallback(game_id, outcome, &imported) {
            if let Err(err) = storage_db::record_play_session(&conn, &measured) {
                tracing::warn!(%err, "cannot record play session");
            }
        }
    }
    *lock(&state.session) = None;
    let error = outcome.error.as_ref().map(|err| SessionErrorPayload {
        kind: err.kind,
        message: err.kind.user_message().to_string(),
        detail: err.detail.clone(),
    });
    tracing::info!(game_id, exit_code = ?outcome.exit_code, "game session ended");
    emit(app, SESSION_ENDED, SessionEndedPayload { game_id, error });
}

/// The launcher-measured session to record when the game started but the runner's
/// own record of *this* session was not imported (runner crashed before writing it).
fn measured_fallback(
    game_id: i64,
    outcome: &SessionOutcome,
    imported: &PendingImportSummary,
) -> Option<NewPlaySession> {
    let started_at = outcome.started_at?;
    if imported.covers_session(game_id, started_at) {
        return None;
    }
    Some(NewPlaySession {
        game_id,
        started_at,
        duration_s: i64::try_from(outcome.measured_seconds).unwrap_or(i64::MAX),
        exit_code: outcome.exit_code,
        source_key: None,
    })
}

#[cfg(test)]
#[path = "play_commands_tests.rs"]
mod tests;
