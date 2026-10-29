//! RetroAchievements account commands (M15 phase 06): password login (the password
//! is used for one request, never stored or logged; only username + token go to
//! `<data>/achievements/credentials.json`), logout, the current account, the
//! runner's login env and the reaction to session achievement updates.

use std::ffi::OsString;
use std::path::Path;
use std::sync::Mutex;

use achievements::{
    delete_credentials, load_credentials, login_with_password, save_credentials, Credentials,
    LoginError, Password, ServerTransport, UreqTransport, TOKEN_ENV, USER_ENV,
};
use config_layers::global_config_path;
use runner_session::AchievementUpdate;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::app_events::{emit, SessionPayload, ACHIEVEMENTS_CHANGED, RA_LOGGED_OUT};
use crate::app_state::{lock, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;
use crate::commands::input_config_view::resolve_global_only;

/// The logged-in account as Settings shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RaAccount {
    pub user: String,
    /// Hardcore / softcore points; known only after a login in this launcher run
    /// (the credentials file holds the username and token only).
    pub points: Option<u32>,
    pub softcore_points: Option<u32>,
}

/// Points of the last login in this run (managed state; never written to disk).
#[derive(Default)]
pub struct RaAccountCache(Mutex<Option<RaAccount>>);

/// Logs in with a password on a blocking task and stores username + token.
#[tauri::command]
pub async fn ra_login(
    app: AppHandle,
    username: String,
    password: String,
) -> CommandResult<RaAccount> {
    // Wrapped at once: the copy is overwritten when dropped (best effort).
    let password = Password::new(password);
    run_blocking(move || {
        let state = app.state::<AppState>();
        let transport = UreqTransport::new(&launcher_user_agent());
        let account = login_and_store(&state.paths.data_root, &transport, &username, &password)?;
        *lock(&app.state::<RaAccountCache>().0) = Some(account.clone());
        Ok(account)
    })
    .await
}

/// Logs out: deletes the credentials file (a running game keeps its session).
#[tauri::command]
pub async fn ra_logout(app: AppHandle) -> CommandResult<()> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        delete_credentials(&state.paths.data_root).map_err(storage_error)?;
        *lock(&app.state::<RaAccountCache>().0) = None;
        tracing::info!("RetroAchievements logout");
        Ok(())
    })
    .await
}

/// The stored login (`None` = logged out), with points when known.
#[tauri::command]
pub fn ra_account(app: AppHandle) -> Option<RaAccount> {
    let state = app.state::<AppState>();
    let cached = lock(&app.state::<RaAccountCache>().0).clone();
    stored_account(&state.paths.data_root, cached)
}

/// `Nostyra/<version> rcheevos/<version>` (no core clause: the launcher runs none).
pub fn launcher_user_agent() -> String {
    format!(
        "Nostyra/{} {}",
        env!("CARGO_PKG_VERSION"),
        achievements::rcheevos_user_agent_clause()
    )
}

/// Login + credentials write, without Tauri (tests use the fake transport).
pub fn login_and_store(
    data_root: &Path,
    transport: &dyn ServerTransport,
    username: &str,
    password: &Password,
) -> CommandResult<RaAccount> {
    let username = username.trim();
    if username.is_empty() {
        return Err(CommandError::invalid(
            "Enter your RetroAchievements username.",
        ));
    }
    let success = login_with_password(transport, username, password).map_err(login_error)?;
    let credentials = Credentials {
        user: success.username.clone(),
        token: success.token.clone(),
    };
    save_credentials(data_root, &credentials).map_err(storage_error)?;
    tracing::info!(user = %success.username, "RetroAchievements login stored");
    Ok(RaAccount {
        user: success.username,
        points: Some(success.score),
        softcore_points: Some(success.score_softcore),
    })
}

/// The account in the credentials file; cached points only for the same user.
pub fn stored_account(data_root: &Path, cached: Option<RaAccount>) -> Option<RaAccount> {
    let credentials = match load_credentials(data_root) {
        Ok(credentials) => credentials?,
        Err(err) => {
            tracing::warn!(%err, "unreadable achievements credentials file");
            return None;
        }
    };
    Some(
        cached
            .filter(|account| account.user.eq_ignore_ascii_case(&credentials.user))
            .unwrap_or(RaAccount {
                user: credentials.user,
                points: None,
                softcore_points: None,
            }),
    )
}

/// `NOSTYRA_RA_USER` / `NOSTYRA_RA_TOKEN` for the runner `Command` when logged in and
/// achievements are enabled (global layer); empty otherwise. Never argv.
pub fn runner_login_env(data_root: &Path) -> Vec<(OsString, OsString)> {
    let (config, _) = resolve_global_only(&global_config_path(data_root));
    if !config.achievements.enabled {
        return Vec::new();
    }
    match load_credentials(data_root) {
        Ok(Some(credentials)) => vec![
            (USER_ENV.into(), credentials.user.into()),
            (TOKEN_ENV.into(), credentials.token.into()),
        ],
        Ok(None) => Vec::new(),
        Err(err) => {
            tracing::warn!(%err, "unreadable achievements credentials file");
            Vec::new()
        }
    }
}

/// A runner achievement event: refresh the game's tab, or log out on a rejected token.
pub fn on_achievement_update(app: &AppHandle, game_id: i64, update: &AchievementUpdate) {
    match update {
        AchievementUpdate::LoginExpired => handle_login_expired(app),
        _ => emit(app, ACHIEVEMENTS_CHANGED, SessionPayload { game_id }),
    }
}

/// The server rejected the stored token: delete it and tell the UI.
pub fn handle_login_expired(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Err(err) = delete_credentials(&state.paths.data_root) {
        tracing::warn!(%err, "cannot delete expired achievements login");
    }
    *lock(&app.state::<RaAccountCache>().0) = None;
    tracing::info!("RetroAchievements login expired; logged out");
    emit(app, RA_LOGGED_OUT, ());
}

/// Login failures as Settings shows them (`login` kind; network kept apart).
fn login_error(err: LoginError) -> CommandError {
    let message = match &err {
        LoginError::InvalidCredentials => "Wrong username or password.".to_owned(),
        LoginError::Network(_) => {
            "Cannot reach RetroAchievements. Check your connection and try again.".to_owned()
        }
        other => format!("Login failed: {other}."),
    };
    let kind = match err {
        LoginError::Network(_) => "network",
        _ => "login",
    };
    CommandError::new(kind, message)
}

fn storage_error(err: std::io::Error) -> CommandError {
    CommandError::new("storage", format!("Cannot update the login file: {err}"))
}

#[cfg(test)]
#[path = "achievements_commands_tests.rs"]
mod tests;
