//! Core status and download (libretro buildbot) for any registry core, validated
//! out of process by `nostyra-runner --probe-core` before it replaces the installed
//! core. One download at a time.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use core_manager::{install_core, CoreInfo, InstalledCore};
use serde::Serialize;
use storage_db::InstalledCoreRecord;
use tauri::{AppHandle, Manager, State};

use crate::app_events::{
    emit, CoreDownloadFinishedPayload, CoreDownloadProgressPayload, CORE_DOWNLOAD_FINISHED,
    CORE_DOWNLOAD_PROGRESS,
};
use crate::app_state::{lock, unix_now, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::core_resolution::core_by_id;
use system_catalog::CoreSpec;

/// At most this often a progress event (downloads read in small chunks).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreStatus {
    pub installed: bool,
    pub path: Option<String>,
    /// From the last download record; `None` for a core placed by hand.
    pub version: Option<String>,
    pub downloading: bool,
}

#[tauri::command]
pub fn get_core_status(state: State<'_, AppState>, core_id: String) -> CommandResult<CoreStatus> {
    let core = core_by_id(&core_id)?;
    let path = core_manager::installed_core(&state.paths.cores_dir, core.id);
    let record = storage_db::get_installed_core(&state.db(), core.id)?;
    Ok(CoreStatus {
        installed: path.is_some(),
        path: path.map(|p| p.to_string_lossy().into_owned()),
        version: record.and_then(|r| r.version),
        downloading: lock(&state.core_download).is_some(),
    })
}

/// Downloads (or re-downloads) `core_id` in the background; returns at once.
/// Refused while a game runs: Windows cannot replace a loaded DLL.
#[tauri::command]
pub fn download_core(
    app: AppHandle,
    state: State<'_, AppState>,
    core_id: String,
) -> CommandResult<()> {
    let core = core_by_id(&core_id)?;
    // Lock order session → core_download (same as play_game), both held while claiming.
    let session = lock(&state.session);
    if session.is_some() {
        return Err(CommandError::new(
            "core",
            "Close the running game before updating the core.",
        ));
    }
    let cancel = {
        let mut slot = lock(&state.core_download);
        if slot.is_some() {
            return Ok(()); // already downloading: the UI just keeps listening
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(cancel.clone());
        cancel
    };
    drop(session);
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = worker_app.state::<AppState>();
        let error = download_and_record(&worker_app, &state, core, &cancel).err();
        *lock(&state.core_download) = None;
        emit(
            &worker_app,
            CORE_DOWNLOAD_FINISHED,
            CoreDownloadFinishedPayload {
                core_id: core.id,
                error,
            },
        );
    });
    Ok(())
}

fn download_and_record(
    app: &AppHandle,
    state: &AppState,
    core: &'static CoreSpec,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let runner = state.paths.runner.clone();
    let probe = move |path: &std::path::Path| {
        runner_session::probe_core_with_runner(&runner, path).map(|core| CoreInfo {
            name: core.name,
            version: core.version,
        })
    };
    let mut last_emit = Instant::now() - PROGRESS_INTERVAL;
    let progress = |bytes_done: u64, bytes_total: Option<u64>| {
        let done = bytes_total == Some(bytes_done);
        if done || last_emit.elapsed() >= PROGRESS_INTERVAL {
            last_emit = Instant::now();
            emit(
                app,
                CORE_DOWNLOAD_PROGRESS,
                CoreDownloadProgressPayload {
                    core_id: core.id,
                    bytes_done,
                    bytes_total,
                },
            );
        }
    };
    let installed = install_core(core.id, &state.paths.cores_dir, probe, progress, cancel)
        .map_err(|err| err.to_string())?;
    tracing::info!(core = %installed.path.display(), version = %installed.info.version, "core installed");
    storage_db::save_installed_core(&state.db(), &record_for(core, &installed))
        .map_err(|err| err.to_string())
}

/// `cores.platform_id` holds the core's primary (first) system; the registry
/// knows every system it plays.
fn record_for(core: &CoreSpec, installed: &InstalledCore) -> InstalledCoreRecord {
    InstalledCoreRecord {
        id: core.id.to_string(),
        platform_id: core.systems[0].to_string(),
        library_path: installed.path.to_string_lossy().into_owned(),
        version: Some(installed.info.version.clone()),
        sha256: installed.sha256.clone(),
        installed_at: unix_now(),
    }
}
