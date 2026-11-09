//! Cloud syncing from the UI: the pre-play pull (Resume flow / Detail slot load,
//! **pull only**, 15 s budget), conflict resolution, the bulk "for all" choice and
//! Settings → Sync now (library pass, progress events `cloud-sync-progress`).

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use cloud_sync::{Choice, SyncOptions};
use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::cloud_app_host::{game_request, start_pass};
use crate::cloud_game_result::SyncGameResult;
use crate::cloud_sync_jobs::{
    run_game_job, run_within_budget, BACKGROUND_CLAIM_WAIT, PRE_PLAY_BUDGET,
};
use crate::cloud_sync_state::JobKind;
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;

/// The user's answer in the conflict dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictChoice {
    /// "Keep this PC".
    KeepLocal,
    /// "Use cloud".
    UseRemote,
}

impl From<ConflictChoice> for Choice {
    fn from(choice: ConflictChoice) -> Self {
        match choice {
            ConflictChoice::KeepLocal => Self::KeepLocal,
            ConflictChoice::UseRemote => Self::UseRemote,
        }
    }
}

/// Before Play: pulls the game's units (+ the shared PS2 memory cards). Pending
/// local changes are not uploaded now (they go after the session). Returns within
/// the 15 s budget: on expiry the job is cancelled and the answer is `Offline`.
#[tauri::command]
pub async fn cloud_sync_game(app: AppHandle, game_id: i64) -> CommandResult<SyncGameResult> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        Ok(
            match game_request(&state, game_id, SyncOptions::pull_only()) {
                Ok(request) => run_within_budget(&app, request, PRE_PLAY_BUDGET),
                Err(skipped) => skipped,
            },
        )
    })
    .await
}

/// The options a conflict answer runs with: `before_play` (Play path dialog) = pull
/// only (Keep this PC uploads nothing now; the choice is kept in the base and
/// uploaded after the session; Use cloud pulls now); else a full sync.
pub fn resolve_options(choice: ConflictChoice, before_play: bool) -> SyncOptions {
    let options = if before_play {
        SyncOptions::pull_only()
    } else {
        SyncOptions::both()
    };
    options.with_choice(choice.into())
}

/// Resolves one game's conflicts with `choice`.
#[tauri::command]
pub async fn cloud_resolve_conflict(
    app: AppHandle,
    game_id: i64,
    choice: ConflictChoice,
    before_play: bool,
) -> CommandResult<SyncGameResult> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let request = match game_request(&state, game_id, resolve_options(choice, before_play)) {
            Ok(request) => request,
            Err(skipped) => return Ok(skipped),
        };
        if before_play {
            return Ok(run_within_budget(&app, request, PRE_PLAY_BUDGET));
        }
        let deadline = Instant::now() + BACKGROUND_CLAIM_WAIT;
        let stop = Arc::new(AtomicBool::new(false));
        Ok(run_game_job(&app, JobKind::Push, deadline, stop, &request))
    })
    .await
}

/// Bulk dialog after Sync now / the startup pass: every game in conflict with one
/// choice, in the background (progress + status events).
#[tauri::command]
pub async fn cloud_resolve_all(app: AppHandle, choice: ConflictChoice) -> CommandResult<()> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let games: Vec<i64> = state
            .cloud
            .status()
            .conflict_games
            .iter()
            .copied()
            .collect();
        if games.is_empty() {
            return Ok(());
        }
        start_pass(&app, Some(&games), Some(choice.into())).map_err(CommandError::invalid)
    })
    .await
}

/// Settings → Sync now: library pass in the background.
#[tauri::command]
pub async fn cloud_sync_now(app: AppHandle) -> CommandResult<()> {
    run_blocking(move || start_pass(&app, None, None).map_err(CommandError::invalid)).await
}
