//! Artwork commands: start/cancel the fetch job, progress, cache stats/clear and the
//! auto-fetch setting.

use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::app_events::ArtworkProgressPayload;
use crate::app_state::{lock, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::artwork_auto_fetch_hook::{auto_fetch_enabled, ARTWORK_AUTO_FETCH_SETTING};
use crate::commands::artwork_fetch_job::start_artwork_job;
use crate::commands::blocking::run_blocking;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkCacheStats {
    pub files: u64,
    pub bytes: u64,
}

/// Fetches all missing art (`gameId` omitted) or one game's (retry window ignored).
#[tauri::command]
pub fn start_artwork_fetch(app: AppHandle, game_id: Option<i64>) -> CommandResult<()> {
    start_artwork_job(&app, game_id)
}

/// Asks the running job to stop (within one request timeout).
#[tauri::command]
pub fn cancel_artwork_fetch(state: State<'_, AppState>) {
    if let Some(cancel) = lock(&state.artwork_cancel).as_ref() {
        cancel.store(true, Ordering::Relaxed);
    }
}

/// Last progress of the running job (`None` = idle).
#[tauri::command]
pub fn get_artwork_progress(state: State<'_, AppState>) -> Option<ArtworkProgressPayload> {
    lock(&state.artwork_progress).clone()
}

#[tauri::command]
pub async fn get_artwork_cache_stats(app: AppHandle) -> CommandResult<ArtworkCacheStats> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let size = artwork_cache::artwork_cache_size(&state.paths.artwork_dir);
        Ok(ArtworkCacheStats {
            files: size.files,
            bytes: size.bytes,
        })
    })
    .await
}

/// Deletes cached images and their rows (the next job re-fetches). Refused while a job
/// runs: a worker could otherwise record a file that was just deleted.
#[tauri::command]
pub async fn clear_artwork_cache(app: AppHandle) -> CommandResult<()> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let job = lock(&state.artwork_cancel);
        if job.is_some() {
            return Err(CommandError::new(
                "artwork",
                "Wait for the artwork fetch to finish (or cancel it).",
            ));
        }
        let removed = artwork_cache::clear_artwork_cache(&state.paths.artwork_dir)?;
        storage_db::clear_artwork(&state.db())?;
        tracing::info!(removed, "artwork cache cleared");
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn get_artwork_auto_fetch(state: State<'_, AppState>) -> CommandResult<bool> {
    Ok(auto_fetch_enabled(&state.db())?)
}

#[tauri::command]
pub fn set_artwork_auto_fetch(state: State<'_, AppState>, enabled: bool) -> CommandResult<()> {
    let value = if enabled { "1" } else { "0" };
    storage_db::set_setting(&state.db(), ARTWORK_AUTO_FETCH_SETTING, value)?;
    Ok(())
}
