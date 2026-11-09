//! Settings → Cloud saves sign-in / sign-out. The Local folder provider has no
//! sign-in (`cloud_set_local_folder`); Google Drive signs in through the system
//! browser + loopback (`cloud_google_drive_account`); OneDrive / Dropbox answer
//! `cloud_unavailable` until their phases (05–06). Sign-out keeps every save, here
//! and in the cloud.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cloud_sync::{client_credentials, OAuthProvider, SignInError};
use config_layers::{global_config_path, set_layer_value, CloudProviderId, Value};
use tauri::{AppHandle, Manager};

use crate::app_events::{emit, CLOUD_STATUS_CHANGED};
use crate::app_state::{lock, AppState};
use crate::cloud_google_drive_account::{
    cloud_http_transport, google_sign_in, store_google_sign_in,
};
use crate::cloud_oauth_account_cleanup::forget_oauth_account;
use crate::cloud_provider_factory::{cloud_config, provider_name};
use crate::cloud_sync_state::CloudStatus;
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;
use crate::commands::cloud_commands::{current_status, sign_in_removal_error, CloudStatusView};

/// Starts the browser sign-in for an OAuth provider; on success the provider is
/// selected and the account stored. Waits up to 5 min (Cancel: `cloud_cancel_sign_in`).
#[tauri::command]
pub async fn cloud_sign_in(
    app: AppHandle,
    provider: CloudProviderId,
) -> CommandResult<CloudStatusView> {
    let credentials = match provider {
        CloudProviderId::GoogleDrive => client_credentials(OAuthProvider::GoogleDrive),
        _ => None,
    }
    .ok_or_else(|| sign_in_refusal(provider))?;
    run_blocking(move || {
        let state = app.state::<AppState>();
        let cancel = Arc::new(AtomicBool::new(false));
        *lock(&state.cloud.sign_in_cancel) = Some(cancel.clone());
        let transport = cloud_http_transport();
        let open_browser = |url: &str| {
            if let Err(err) = tauri_plugin_opener::open_url(url, None::<&str>) {
                tracing::warn!(target: "cloud_sync", %err, "cannot open the browser for sign-in");
            }
        };
        let result = google_sign_in(transport.as_ref(), credentials, open_browser, &cancel);
        {
            // A newer sign-in may own the slot now: leave its Cancel reachable.
            let mut slot = lock(&state.cloud.sign_in_cancel);
            if slot.as_ref().is_some_and(|held| Arc::ptr_eq(held, &cancel)) {
                *slot = None;
            }
        }
        let signed = result.map_err(sign_in_error)?;
        store_google_sign_in(&state.paths.data_root, &signed).map_err(|err| {
            CommandError::new("storage", format!("Cannot store the sign-in: {err}"))
        })?;
        *lock(&state.cloud.account) = None;
        *state.cloud.status() = CloudStatus::default();
        tracing::info!(target: "cloud_sync", provider = provider.as_str(), "cloud saves signed in");
        emit(&app, CLOUD_STATUS_CHANGED, serde_json::json!({}));
        Ok(current_status(&state))
    })
    .await
}

/// Why `cloud_sign_in(provider)` cannot run in this version.
pub fn sign_in_refusal(provider: CloudProviderId) -> CommandError {
    match provider {
        CloudProviderId::Off => CommandError::invalid("Choose a cloud provider first."),
        CloudProviderId::LocalFolder => {
            CommandError::invalid("The Local folder needs no sign-in: choose the folder.")
        }
        http => CommandError::new(
            "cloud_unavailable",
            format!(
                "{} sign-in is not available in this version yet.",
                provider_name(http)
            ),
        ),
    }
}

/// A failed sign-in for the UI (messages carry no codes / tokens).
pub fn sign_in_error(err: SignInError) -> CommandError {
    match err {
        SignInError::Cancelled => CommandError::invalid("Sign-in cancelled."),
        SignInError::TimedOut => CommandError::invalid("Sign-in timed out. Try again."),
        other => CommandError::new("cloud_sign_in", other.to_string()),
    }
}

/// Cancels a waiting browser sign-in (no-op when none runs).
#[tauri::command]
pub fn cloud_cancel_sign_in(state: tauri::State<'_, AppState>) {
    if let Some(cancel) = lock(&state.cloud.sign_in_cancel).as_ref() {
        cancel.store(true, Ordering::SeqCst);
    }
}

/// Turns cloud saves off for this PC: any stored OAuth account is deleted, whichever
/// provider is picked (Google: the refresh token is revoked first, best effort), the Local folder path is kept for an
/// easy re-pick. Saves stay on this PC and in the cloud; base manifests stay too, so
/// signing in to the same account again resumes without conflicts.
#[tauri::command]
pub async fn cloud_sign_out(app: AppHandle) -> CommandResult<CloudStatusView> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let data_root = &state.paths.data_root;
        let provider = cloud_config(data_root).provider;
        // Whatever provider is picked: a Google token left by an earlier pick goes too.
        forget_oauth_account(data_root).map_err(sign_in_removal_error)?;
        let off = Value::from(CloudProviderId::Off.as_str());
        set_layer_value(&global_config_path(data_root), "cloud.provider", Some(off))?;
        *lock(&state.cloud.account) = None;
        *state.cloud.status() = CloudStatus::default();
        tracing::info!(target: "cloud_sync", provider = provider.as_str(), "cloud saves signed out");
        emit(&app, CLOUD_STATUS_CHANGED, serde_json::json!({}));
        Ok(current_status(&state))
    })
    .await
}
