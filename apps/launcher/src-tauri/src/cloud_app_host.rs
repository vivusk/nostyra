//! Tauri side of the cloud jobs: the app handle as [`CloudHost`], game / library
//! lookups in the database, and the triggers — push after a session, the launcher
//! start pass, Sync now — plus `setup` (startup pass + main window close-wait).

use std::path::Path;

use cloud_sync::{Choice, SyncOptions};
use storage_db::GameListQuery;
use tauri::{AppHandle, Manager};

use crate::app_events::emit;
use crate::app_state::{lock, AppState};
use crate::cloud_close_wait::on_close_requested;
use crate::cloud_game_result::SyncGameResult;
use crate::cloud_game_sync::{CloudCtx, CloudHost};
use crate::cloud_library_pass::{PassGame, PassRequest};
use crate::cloud_provider_factory::{
    cloud_config, current_account, sync_scope, AccountState, CloudAccount,
};
use crate::cloud_sync_jobs::{spawn_game_job, spawn_pass, GameSyncRequest};
use crate::cloud_sync_state::{CloudSyncState, JobKind};
use crate::cloud_unit_guard::game_sync_units;
use crate::commands::game_data_target::GameDataTarget;

impl CloudHost for AppHandle {
    fn ctx(&self) -> CloudCtx<'_> {
        let state = self.state::<AppState>().inner();
        CloudCtx {
            data_root: &state.paths.data_root,
            session: &state.session,
            cloud: &state.cloud,
        }
    }

    fn notify(&self, event: &str, payload: serde_json::Value) {
        emit(self, event, payload);
    }
}

/// The signed-in account, or the `Skipped` reason the UI shows.
pub fn ready_account(state: &AppState) -> Result<CloudAccount, SyncGameResult> {
    ready_account_in(&state.paths.data_root, &state.cloud)
}

/// [`ready_account`] without the app state (tests).
pub fn ready_account_in(
    data_root: &Path,
    cloud: &CloudSyncState,
) -> Result<CloudAccount, SyncGameResult> {
    let config = cloud_config(data_root);
    let reason = match current_account(data_root, cloud, &config) {
        AccountState::Ready(account) => return Ok(account),
        AccountState::Off => "signed out",
        AccountState::NeedsSignIn(reason) => reason,
        AccountState::Unavailable(_) => "provider not available yet",
    };
    Err(skipped(reason))
}

/// What syncing `game_id` with `options` needs, or why it is skipped (signed out,
/// no content id, …).
pub fn game_request(
    state: &AppState,
    game_id: i64,
    options: SyncOptions,
) -> Result<GameSyncRequest, SyncGameResult> {
    let target = GameDataTarget::load(state, game_id).map_err(|err| skipped(&err.message))?;
    request_for(
        &state.paths.data_root,
        &state.cloud,
        game_id,
        &target,
        options,
    )
}

/// [`game_request`] for an already loaded game (tests: no database).
pub fn request_for(
    data_root: &Path,
    cloud: &CloudSyncState,
    game_id: i64,
    target: &GameDataTarget,
    options: SyncOptions,
) -> Result<GameSyncRequest, SyncGameResult> {
    let account = ready_account_in(data_root, cloud)?;
    let units = game_sync_units(data_root, target).map_err(skipped)?;
    Ok(GameSyncRequest {
        game_id,
        account,
        scope: sync_scope(&cloud_config(data_root)),
        units,
        options,
    })
}

fn skipped(reason: &str) -> SyncGameResult {
    SyncGameResult::Skipped {
        reason: reason.to_string(),
    }
}

/// Every library game with sync units (`ids` = only these).
pub fn library_games(state: &AppState, ids: Option<&[i64]>) -> Vec<PassGame> {
    let games = match storage_db::list_games(&state.db(), &GameListQuery::default()) {
        Ok(games) => games,
        Err(err) => {
            tracing::warn!(target: "cloud_sync", %err, "cannot list games");
            return Vec::new();
        }
    };
    games
        .iter()
        .filter(|game| ids.is_none_or(|ids| ids.contains(&game.id)))
        .filter_map(|game| {
            let target = GameDataTarget::from_game(game);
            let units = game_sync_units(&state.paths.data_root, &target).ok()?;
            Some(PassGame {
                game_id: game.id,
                units,
            })
        })
        .collect()
}

/// After `finish_session` freed the slot: push the game's units in the background.
pub fn spawn_push_after_session(app: &AppHandle, game_id: i64) {
    let state = app.state::<AppState>();
    match game_request(&state, game_id, SyncOptions::both()) {
        Ok(request) => spawn_game_job(app.clone(), JobKind::Push, request),
        Err(SyncGameResult::Skipped { reason }) => {
            tracing::debug!(target: "cloud_sync", game_id, %reason, "no push after the session");
        }
        Err(_) => {}
    }
}

/// A pass over the library (`games` = `None`) or some games, with an optional
/// bulk choice. `Err(reason)` when there is no usable account.
pub fn start_pass(
    app: &AppHandle,
    games: Option<&[i64]>,
    choice: Option<Choice>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let account = match ready_account(&state) {
        Ok(account) => account,
        Err(SyncGameResult::Skipped { reason }) => return Err(reason),
        Err(_) => return Err("cloud unavailable".to_string()),
    };
    let scope = sync_scope(&cloud_config(&state.paths.data_root));
    let (handle, ids) = (app.clone(), games.map(<[i64]>::to_vec));
    // The library enumeration (db + a `read_dir` per game) runs on the pass thread:
    // the startup trigger calls this from the Tauri `setup` hook (cold start, F2).
    spawn_pass(app.clone(), account, scope, move || PassRequest {
        games: library_games(&handle.state::<AppState>(), ids.as_deref()),
        choice,
        only_changed: ids.is_none() && choice.is_none(),
    });
    Ok(())
}

/// Launcher start: background pass unless `NOSTYRA_OFFLINE=1`, turned off, a game
/// runs or nothing is signed in.
fn spawn_startup_cloud_sync(app: &AppHandle, data_root: &Path) {
    let skip = if std::env::var("NOSTYRA_OFFLINE").is_ok_and(|value| value == "1") {
        Some("offline mode")
    } else if !cloud_config(data_root).sync_on_start {
        Some("turned off")
    } else if lock(&app.state::<AppState>().session).is_some() {
        Some("a game is running")
    } else {
        None
    };
    if let Some(reason) = skip {
        tracing::info!(target: "cloud_sync", reason, "startup cloud sync skipped");
        return;
    }
    if let Err(reason) = start_pass(app, None, None) {
        tracing::info!(target: "cloud_sync", %reason, "startup cloud sync skipped");
    }
}

/// `lib.rs` setup hook: startup pass + close-wait on the main window.
pub fn setup(app: &AppHandle) {
    let data_root = app.state::<AppState>().paths.data_root.clone();
    spawn_startup_cloud_sync(app, &data_root);
    let Some(window) = app.get_webview_window("main") else {
        tracing::warn!("no main window: cloud close-wait not installed");
        return;
    };
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            on_close_requested(&handle, api);
        }
    });
}
