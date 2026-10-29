//! Auto-fetch after a scan: new games get art without the user asking (setting
//! `artwork_auto_fetch`, on unless set to "0").

use storage_db::{Connection, StorageResult};
use tauri::AppHandle;

use crate::app_state::AppState;
use crate::commands::artwork_fetch_job::start_artwork_job;

pub const ARTWORK_AUTO_FETCH_SETTING: &str = "artwork_auto_fetch";

/// Auto-fetch is on unless the setting is exactly "0".
pub fn auto_fetch_enabled(conn: &Connection) -> StorageResult<bool> {
    Ok(storage_db::get_setting(conn, ARTWORK_AUTO_FETCH_SETTING)?.as_deref() != Some("0"))
}

/// Called when a scan finished: starts (or queues a rerun of) the missing-art job.
pub fn maybe_start_artwork_fetch(app: &AppHandle, state: &AppState, scan_cancelled: bool) {
    if scan_cancelled {
        return;
    }
    match auto_fetch_enabled(&state.db()) {
        Ok(true) => {
            if let Err(err) = start_artwork_job(app, None) {
                tracing::warn!(err = %err.message, "artwork auto-fetch not started");
            }
        }
        Ok(false) => {}
        Err(err) => tracing::warn!(%err, "cannot read the artwork auto-fetch setting"),
    }
}
