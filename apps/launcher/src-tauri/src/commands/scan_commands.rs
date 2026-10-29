//! Library scan over all folders on a worker thread, with progress events and cancel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use game_library::{scan_library, ScanProgress, ScanSummary};
use tauri::{AppHandle, Manager, State};

use crate::app_events::{
    emit, ScanFinishedPayload, ScanProblem, ScanProgressPayload, MAX_SCAN_PROBLEMS, SCAN_FINISHED,
    SCAN_PROGRESS,
};
use crate::app_state::{lock, unix_now, AppState};
use crate::command_error::CommandResult;
use crate::commands::artwork_auto_fetch_hook::maybe_start_artwork_fetch;
use crate::relinked_game_files::migrate_relinked_game_files;

/// Starts scanning every library folder; returns at once. While a scan runs, a new
/// request (e.g. a folder was just added) is queued: the scan runs once more at the end.
#[tauri::command]
pub fn start_scan(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    let cancel = {
        let mut slot = lock(&state.scan_cancel);
        if slot.is_some() {
            state.scan_rerun.store(true, Ordering::Relaxed);
            return Ok(());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(cancel.clone());
        cancel
    };
    // Visible at once to views that ask before the first progress event.
    *lock(&state.scan_progress) = Some(ScanProgressPayload::default());
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = worker_app.state::<AppState>();
        let mut total = ScanFinishedPayload::default();
        let mut errors = Vec::new();
        loop {
            // Per pass: a queued rerun re-sees unchanged files, so skipped/failed and
            // folder errors describe the last pass; added/updated/missing/relinked add up.
            total.skipped = 0;
            total.failed_files = 0;
            errors.clear();
            scan_all_folders(&worker_app, &state, &cancel, &mut total, &mut errors);
            // Checked under the scan_cancel lock, so a request queued by start_scan
            // is either seen here or starts a fresh scan: never lost.
            let mut slot = lock(&state.scan_cancel);
            if !cancel.load(Ordering::Relaxed) && state.scan_rerun.swap(false, Ordering::Relaxed) {
                continue;
            }
            state.scan_rerun.store(false, Ordering::Relaxed);
            *lock(&state.scan_progress) = None;
            *slot = None;
            break;
        }
        total.cancelled = cancel.load(Ordering::Relaxed);
        total.error = (!errors.is_empty()).then(|| errors.join("\n"));
        let cancelled = total.cancelled;
        emit(&worker_app, SCAN_FINISHED, total);
        maybe_start_artwork_fetch(&worker_app, &state, cancelled);
    });
    Ok(())
}

/// Asks the running scan to stop after its current batch.
#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) {
    if let Some(cancel) = lock(&state.scan_cancel).as_ref() {
        cancel.store(true, Ordering::Relaxed);
    }
}

/// Last progress of the running scan (`None` = idle), for views opened mid-scan.
#[tauri::command]
pub fn get_scan_progress(state: State<'_, AppState>) -> Option<ScanProgressPayload> {
    lock(&state.scan_progress).clone()
}

/// One pass over the current folders (two-pass scan, see `game_library::scan_library`);
/// adds to `total`, one readable line per failed folder. Then moves the save folder and
/// game config of renamed games to their new ROM key.
fn scan_all_folders(
    app: &AppHandle,
    state: &AppState,
    cancel: &AtomicBool,
    total: &mut ScanFinishedPayload,
    errors: &mut Vec<String>,
) {
    let folders = match storage_db::list_library_folders(&state.db()) {
        Ok(folders) => folders,
        Err(err) => {
            errors.push(err.to_string());
            return;
        }
    };
    let report = |progress: &ScanProgress| {
        let payload = ScanProgressPayload {
            folder_path: progress.folder_path.clone(),
            files_total: progress.files_total,
            files_done: progress.files_done,
            current_path: progress.current_path.clone(),
        };
        *lock(&state.scan_progress) = Some(payload.clone());
        emit(app, SCAN_PROGRESS, payload);
    };
    let summary = match scan_library(
        &state.db,
        &folders,
        &state.catalogs,
        unix_now(),
        &report,
        cancel,
    ) {
        Ok(summary) => summary,
        Err(err) => {
            tracing::warn!(%err, "library scan failed");
            errors.push(err.to_string());
            return;
        }
    };
    for (folder, error) in &summary.folder_errors {
        tracing::warn!(%folder, %error, "folder scan failed");
        errors.push(folder_error(folder, error));
    }
    add_summary(total, &summary);
    if !summary.relinked_games.is_empty() {
        // Session lock held for the whole move: Play cannot start a game mid-move.
        let session = lock(&state.session);
        migrate_relinked_game_files(
            &state.paths.data_root,
            &state.db(),
            &summary.relinked_games,
            session.map(|s| s.game_id),
        );
    }
}

/// "E:\ROMs: library folder not found…" — the folder is named even when the error is not.
fn folder_error(folder: &str, error: &str) -> String {
    if error.contains(folder) {
        error.to_string()
    } else {
        format!("{folder}: {error}")
    }
}

fn add_summary(total: &mut ScanFinishedPayload, summary: &ScanSummary) {
    total.added += summary.added;
    total.updated += summary.updated;
    total.missing += summary.missing;
    total.relinked += summary.relinked;
    total.skipped += summary.skipped;
    total.failed_files += summary.failures.len();
    for failure in &summary.failures {
        tracing::info!(path = %failure.path.display(), reason = %failure.reason, "file not scanned");
        if total.failures.len() < MAX_SCAN_PROBLEMS {
            total.failures.push(ScanProblem {
                path: failure.path.display().to_string(),
                reason: failure.reason.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_errors_name_the_folder_once() {
        assert_eq!(
            folder_error("E:/ROMs", "database is locked"),
            "E:/ROMs: database is locked"
        );
        assert_eq!(
            folder_error("E:/ROMs", "library folder not found: E:/ROMs"),
            "library folder not found: E:/ROMs"
        );
    }

    #[test]
    fn summaries_add_up_across_passes() {
        let mut total = ScanFinishedPayload::default();
        let summary = ScanSummary {
            added: 2,
            relinked: 1,
            skipped: 3,
            ..ScanSummary::default()
        };
        add_summary(&mut total, &summary);
        add_summary(&mut total, &summary);
        assert_eq!((total.added, total.relinked, total.skipped), (4, 2, 6));
    }
}
