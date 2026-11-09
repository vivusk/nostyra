//! Update checker commands (Settings → Updates + the startup banner). The automatic
//! check runs inside `get_update_status` at most once a day; failures are silent and
//! the cached answer is served. The DB lock is never held across the HTTP request.

use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use crate::app_state::{unix_now, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;
use crate::update_check_settings as settings;
use crate::update_check_status::{automatic_skip_reason, build_status, UpdateStatus};
use crate::update_release_feed::{fetch_latest_release, FeedOutcome};
use crate::update_release_rules::{safe_release_url, RELEASES_LATEST_URL};

/// Serializes feed requests so the automatic check and "Check now" never overlap.
static CHECK_LOCK: Mutex<()> = Mutex::new(());

/// Status, running the automatic check first when it is on, due and not a debug build.
#[tauri::command]
pub async fn get_update_status(app: AppHandle) -> CommandResult<UpdateStatus> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let now = unix_now();
        let skip_reason = {
            let conn = state.db();
            let enabled = settings::update_check_enabled(&conn)?;
            let due = settings::is_due(now, settings::last_checked_at(&conn)?);
            automatic_skip_reason(enabled, cfg!(debug_assertions), due)
        };
        match skip_reason {
            Some(reason) => tracing::info!("update check skipped reason={reason}"),
            None => {
                run_update_check(&state, now);
            }
        }
        let status = build_status(&state.db(), false)?;
        Ok(status)
    })
    .await
}

/// "Check now": ignores toggle, throttle and debug; a failure is reported in the status.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> CommandResult<UpdateStatus> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let ok = run_update_check(&state, unix_now());
        let status = build_status(&state.db(), !ok)?;
        Ok(status)
    })
    .await
}

#[tauri::command]
pub fn set_update_check_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<UpdateStatus> {
    let conn = state.db();
    settings::set_update_check_enabled(&conn, enabled)?;
    Ok(build_status(&conn, false)?)
}

#[tauri::command]
pub fn skip_update_version(
    state: State<'_, AppState>,
    version: String,
) -> CommandResult<UpdateStatus> {
    let conn = state.db();
    if !settings::set_skipped_version(&conn, &version)? {
        return Err(CommandError::invalid(format!("Not a version: {version}")));
    }
    Ok(build_status(&conn, false)?)
}

/// Opens the cached release page (allowlisted) or the generic latest-release page.
#[tauri::command]
pub async fn open_update_release_page(app: AppHandle) -> CommandResult<()> {
    run_blocking(move || {
        let cached = settings::load_update_cache(&app.state::<AppState>().db())?;
        let url = cached.map_or_else(|| RELEASES_LATEST_URL.to_owned(), |(r, _)| r.url);
        tauri_plugin_opener::open_url(safe_release_url(&url), None::<&str>)
            .map_err(|err| CommandError::new("storage", format!("Cannot open the browser: {err}")))
    })
    .await
}

/// One feed request (conditional when a valid cached release exists). Returns true on
/// 200 / 304 (both stamp the check time); failures stamp nothing, so the next launch
/// retries.
fn run_update_check(state: &AppState, now: i64) -> bool {
    // A poisoned lock only means an earlier check panicked; the guard data is `()`.
    let _serialized = CHECK_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let etag = match settings::load_update_cache(&state.db()) {
        Ok(cached) => cached.and_then(|(_, etag)| etag),
        Err(err) => {
            tracing::warn!(%err, "cannot read the update cache");
            None
        }
    };
    let stored = match fetch_latest_release(&state.http, etag.as_deref()) {
        Ok(FeedOutcome::Fresh { release, etag }) => {
            tracing::info!("update check status=200");
            settings::store_fresh(&state.db(), &release, etag.as_deref(), now)
        }
        Ok(FeedOutcome::NotModified) => {
            tracing::info!("update check status=304");
            settings::stamp_checked(&state.db(), now)
        }
        Err(err) => {
            tracing::info!(detail = %err, "update check failed class={}", err.class());
            return false;
        }
    };
    if let Err(err) = &stored {
        tracing::warn!(%err, "cannot store the update check result");
    }
    stored.is_ok()
}
