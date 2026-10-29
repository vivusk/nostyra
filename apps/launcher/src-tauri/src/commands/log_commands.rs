//! Settings → About → Logs: open the logs folder, and create a bug-report zip (also
//! offered on game error toasts). The zip is revealed in the file manager so the user
//! can drag it into a GitHub issue; nothing is uploaded.

use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::bug_report_system_info::{gather_system_info, system_info_text};
use crate::bug_report_zip::write_bug_report;
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;

/// Opens `<data>/logs/` in the OS file manager (created when missing).
#[tauri::command]
pub async fn open_logs_folder(app: AppHandle) -> CommandResult<()> {
    run_blocking(move || {
        let dir = ipc_protocol::log_files::logs_dir(&app.state::<AppState>().paths.data_root);
        std::fs::create_dir_all(&dir).map_err(|err| {
            CommandError::new("storage", format!("Cannot create the logs folder: {err}"))
        })?;
        tauri_plugin_opener::open_path(&dir, None::<&str>)
            .map_err(|err| CommandError::new("storage", format!("Cannot open the folder: {err}")))
    })
    .await
}

/// Writes `<data>/logs/nostyra-bug-report-<stamp>.zip`, reveals it in the file manager
/// and returns its path (a failed reveal still returns the path: the zip exists).
#[tauri::command]
pub async fn create_bug_report(app: AppHandle) -> CommandResult<String> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let info = system_info_text(&gather_system_info(&state));
        let dir = ipc_protocol::log_files::logs_dir(&state.paths.data_root);
        let path = write_bug_report(&dir, &info).map_err(storage_error)?;
        tracing::info!(zip = %path.display(), "bug report created");
        if let Err(err) = tauri_plugin_opener::reveal_item_in_dir(&path) {
            tracing::warn!(%err, "cannot reveal the bug report");
        }
        Ok(path.to_string_lossy().into_owned())
    })
    .await
}

fn storage_error(err: std::io::Error) -> CommandError {
    CommandError::new("storage", format!("Cannot write the bug report: {err}"))
}
