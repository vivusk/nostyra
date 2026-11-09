//! Settings → Cloud saves: status, the Local folder choice, the Detail chip state.
//! Sign-in lives in `cloud_sign_in_commands`, syncing in `cloud_sync_commands`.

use std::path::{Path, PathBuf};

use cloud_sync::BaseManifestStore;
use config_layers::{global_config_path, set_layer_values, CloudConfig, CloudProviderId, Value};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::cloud_change_detection::local_dirty;
use crate::cloud_oauth_account_cleanup::forget_oauth_account;
use crate::cloud_provider_factory::{
    cloud_config, current_account, provider_name, sync_scope, AccountState,
};
use crate::cloud_sync_state::CloudStatus;
use crate::cloud_unit_guard::game_sync_units;
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;
use crate::commands::game_data_target::GameDataTarget;

/// What Settings shows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatusView {
    pub provider: CloudProviderId,
    /// The provider is built into this version (HTTP providers: phases 04–06).
    pub provider_available: bool,
    pub account_label: Option<String>,
    /// `provider != off && !needs_sign_in` (Local folder: configured + reachable).
    pub signed_in: bool,
    pub needs_sign_in: bool,
    /// Why sign-in is needed ("The cloud folder is not reachable.", …).
    pub sign_in_reason: Option<String>,
    pub local_folder: String,
    pub sync_states: bool,
    pub sync_on_start: bool,
    pub syncing: bool,
    pub last_sync_at: Option<i64>,
    pub last_result: Option<String>,
    pub pending_units: usize,
    pub conflict_games: Vec<i64>,
}

/// The Settings view from the config, the account and the last results.
pub fn status_view(
    config: &CloudConfig,
    account: &AccountState,
    status: &CloudStatus,
    syncing: bool,
) -> CloudStatusView {
    let (label, reason, available) = match account {
        AccountState::Off => (None, None, true),
        AccountState::Ready(account) => (Some(account.label.clone()), None, true),
        AccountState::NeedsSignIn(reason) => (None, Some((*reason).to_string()), true),
        AccountState::Unavailable(id) => {
            let reason = format!(
                "{} is not available in this version yet.",
                provider_name(*id)
            );
            (None, Some(reason), false)
        }
    };
    let reason = reason.or_else(|| status.auth_expired.then(|| "Sign in again.".to_string()));
    let needs_sign_in = config.provider != CloudProviderId::Off && reason.is_some();
    CloudStatusView {
        provider: config.provider,
        provider_available: available,
        account_label: label,
        signed_in: config.provider != CloudProviderId::Off && !needs_sign_in,
        needs_sign_in,
        sign_in_reason: reason,
        local_folder: config.local_folder.clone(),
        sync_states: config.sync_states,
        sync_on_start: config.sync_on_start,
        syncing,
        last_sync_at: status.last_sync_at,
        last_result: status.last_result.clone(),
        pending_units: status.pending_units,
        conflict_games: status.conflict_games.iter().copied().collect(),
    }
}

/// Current view (shared by the commands that change it).
pub fn current_status(state: &AppState) -> CloudStatusView {
    let config = cloud_config(&state.paths.data_root);
    let account = current_account(&state.paths.data_root, &state.cloud, &config);
    let syncing = state.cloud.job_running();
    status_view(&config, &account, &state.cloud.status(), syncing)
}

#[tauri::command]
pub async fn cloud_status(app: AppHandle) -> CommandResult<CloudStatusView> {
    run_blocking(move || Ok(current_status(&app.state::<AppState>()))).await
}

/// Chooses the Local folder provider over `path` (must exist, be writable and lie
/// outside the data folder). A stored OAuth sign-in is revoked + deleted first: it
/// would otherwise stay valid on disk with nothing using it.
#[tauri::command]
pub async fn cloud_set_local_folder(
    app: AppHandle,
    path: String,
) -> CommandResult<CloudStatusView> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let folder = validate_local_folder(&state.paths.data_root, &path)?;
        forget_oauth_account(&state.paths.data_root).map_err(sign_in_removal_error)?;
        let entries = [
            (
                "cloud.provider",
                Some(Value::from(CloudProviderId::LocalFolder.as_str())),
            ),
            (
                "cloud.local_folder",
                Some(Value::from(folder.to_string_lossy().into_owned())),
            ),
        ];
        set_layer_values(&global_config_path(&state.paths.data_root), &entries)?;
        *state.cloud.status() = CloudStatus::default();
        tracing::info!(target: "cloud_sync", "local cloud folder chosen");
        Ok(current_status(&state))
    })
    .await
}

/// A stored sign-in that could not be deleted.
pub fn sign_in_removal_error(err: std::io::Error) -> CommandError {
    CommandError::new("storage", format!("Cannot remove the sign-in: {err}"))
}

/// The folder a Local folder account may use.
pub fn validate_local_folder(data_root: &Path, path: &str) -> CommandResult<PathBuf> {
    let folder = PathBuf::from(path.trim());
    if !folder.is_absolute() || !folder.is_dir() {
        return Err(CommandError::invalid("Choose an existing folder."));
    }
    let canonical = std::fs::canonicalize(&folder).unwrap_or_else(|_| folder.clone());
    let data = std::fs::canonicalize(data_root).unwrap_or_else(|_| data_root.to_path_buf());
    if canonical.starts_with(&data) {
        return Err(CommandError::invalid(
            "Choose a folder outside Nostyra's data folder.",
        ));
    }
    let probe = folder.join(format!(".nostyra-write-test-{}", std::process::id()));
    let writable = std::fs::write(&probe, b"x").is_ok();
    let _ = std::fs::remove_file(&probe);
    if !writable {
        return Err(CommandError::invalid(
            "Nostyra cannot write to this folder.",
        ));
    }
    Ok(folder)
}

/// Detail chip state of one game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCloudStatus {
    /// "off" | "synced" | "pending" | "conflict" | "error".
    pub state: &'static str,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn cloud_game_status(app: AppHandle, game_id: i64) -> CommandResult<GameCloudStatus> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let data_root = &state.paths.data_root;
        let config = cloud_config(data_root);
        let AccountState::Ready(account) = current_account(data_root, &state.cloud, &config) else {
            return Ok(chip("off", None));
        };
        let status = state.cloud.status().clone();
        if status.conflict_games.contains(&game_id) {
            return Ok(chip("conflict", None));
        }
        if let Some(error) = status.game_errors.get(&game_id) {
            return Ok(chip("error", Some(error.clone())));
        }
        let target = GameDataTarget::load(&state, game_id)?;
        let Ok(units) = game_sync_units(data_root, &target) else {
            return Ok(chip("off", None));
        };
        let bases = BaseManifestStore::new(data_root, &account.account_key);
        let scope = sync_scope(&config);
        let pending = units.iter().any(|unit| {
            let base = bases.load(unit).ok().flatten();
            local_dirty(unit, base.as_ref(), &scope)
        });
        Ok(chip(if pending { "pending" } else { "synced" }, None))
    })
    .await
}

fn chip(state: &'static str, error: Option<String>) -> GameCloudStatus {
    GameCloudStatus { state, error }
}
