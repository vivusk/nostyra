//! Settings → Data folder: check a picked folder, then switch to it (optionally moving
//! the current data there) and restart. The data root is fixed for the process
//! (database, asset scope, runner `--data-dir`), so a restart is the switch.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::app_events::{emit, DATA_FOLDER_MOVE_PROGRESS};
use crate::app_state::{lock, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;
use crate::data_folder_move::{
    copy_planned_files, folders_overlap, plan_move, remove_moved_originals,
};

/// What the confirm dialog needs about a picked folder.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFolderCheck {
    /// Files of the current data root that a move would copy.
    pub files: usize,
    /// Of those, files that already exist in the picked folder.
    pub conflicts: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFolderMoveProgress {
    pub done: usize,
    pub total: usize,
}

/// Validates `path` as a new data root and counts what a move would copy.
#[tauri::command]
pub async fn check_data_folder(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<DataFolderCheck> {
    let current = state.paths.data_root.clone();
    run_blocking(move || {
        let target = validate_target(&current, &path)?;
        let plan = plan_move(&current, &target).map_err(storage_error)?;
        Ok(DataFolderCheck {
            files: plan.files.len(),
            conflicts: plan.conflicts,
        })
    })
    .await
}

/// Switches the data root to `path` and restarts. With `move_data`, the current data is
/// copied there first (`overwrite`: replace files that exist there, else keep them and
/// leave those originals behind); a failed copy keeps the current root.
#[tauri::command]
pub async fn change_data_folder(
    app: AppHandle,
    path: String,
    move_data: bool,
    overwrite: bool,
) -> CommandResult<()> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        ensure_idle(&state)?;
        let current = state.paths.data_root.clone();
        let target = validate_target(&current, &path)?;
        let location_file = ipc_protocol::data_location_file()
            .ok_or_else(|| CommandError::invalid("Cannot find the user settings folder."))?;
        if move_data {
            // Held until the restart: nothing reads or writes the library meanwhile.
            let mut db = state.db();
            // Close the database so its files copy whole (closing checkpoints the WAL
            // and deletes `-wal` / `-shm`, so the plan is made after this).
            let open = std::mem::replace(&mut *db, storage_db::open_in_memory_database()?);
            if let Err((still_open, err)) = open.close() {
                *db = still_open; // keep serving the library as before
                return Err(CommandError::new("storage", err.to_string()));
            }
            let reopen = |db: &mut storage_db::Connection| -> CommandResult<()> {
                *db = storage_db::open_database(&current.join("nostyra.db"))?;
                Ok(())
            };
            let plan = match plan_move(&current, &target) {
                Ok(plan) => plan,
                Err(err) => {
                    reopen(&mut db)?;
                    return Err(storage_error(err));
                }
            };
            let copied =
                match copy_planned_files(&current, &target, &plan, overwrite, |done, total| {
                    // Every 25 files is plenty for a progress bar.
                    if done % 25 == 0 || done == total {
                        emit(
                            &app,
                            DATA_FOLDER_MOVE_PROGRESS,
                            DataFolderMoveProgress { done, total },
                        );
                    }
                }) {
                    Ok(copied) => copied,
                    Err(err) => {
                        reopen(&mut db)?;
                        return Err(CommandError::new(
                            "storage",
                            format!("Moving the data stopped; nothing changed. {err}"),
                        ));
                    }
                };
            if let Err(err) = ipc_protocol::write_data_location(&location_file, &target) {
                reopen(&mut db)?;
                return Err(storage_error(err));
            }
            remove_moved_originals(&current, &copied);
            tracing::info!(to = %target.display(), files = copied.len(), "data folder moved");
            app.restart();
        }
        ipc_protocol::write_data_location(&location_file, &target).map_err(storage_error)?;
        tracing::info!(to = %target.display(), "data folder changed");
        app.restart();
    })
    .await
}

/// Refuses while anything uses the data root (a game, scan, artwork fetch, core download).
fn ensure_idle(state: &AppState) -> CommandResult<()> {
    let busy = if lock(&state.session).is_some() {
        Some("a game is running")
    } else if lock(&state.scan_cancel).is_some() {
        Some("the library is being scanned")
    } else if lock(&state.artwork_cancel).is_some() {
        Some("artwork is downloading")
    } else if lock(&state.core_download).is_some() {
        Some("a core is downloading")
    } else {
        None
    };
    match busy {
        Some(what) => Err(CommandError::new(
            "busy",
            format!("Cannot change the data folder while {what}."),
        )),
        None => Ok(()),
    }
}

/// An existing, writable folder that neither contains nor sits inside the current root.
fn validate_target(current: &Path, path: &str) -> CommandResult<PathBuf> {
    let target = PathBuf::from(path);
    if !target.is_dir() {
        return Err(CommandError::invalid("That folder does not exist."));
    }
    if folders_overlap(current, &target) {
        return Err(CommandError::invalid(
            "Choose a folder outside the current data folder (and not one that contains it).",
        ));
    }
    let probe = target.join(".nostyra-write-test");
    fs::write(&probe, b"")
        .and_then(|()| fs::remove_file(&probe))
        .map_err(|err| CommandError::invalid(format!("Cannot write to that folder: {err}")))?;
    Ok(target)
}

fn storage_error(err: std::io::Error) -> CommandError {
    CommandError::new("storage", err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_must_exist_and_not_overlap() {
        let root = tempfile::tempdir().unwrap();
        let current = root.path().join("data");
        let inside = current.join("sub");
        let other = root.path().join("other");
        fs::create_dir_all(&inside).unwrap();
        fs::create_dir_all(&other).unwrap();
        let text = |p: &Path| p.to_str().unwrap().to_string();

        assert_eq!(validate_target(&current, &text(&other)).unwrap(), other);
        assert!(!other.join(".nostyra-write-test").exists());
        for bad in [&inside, &current, &root.path().to_path_buf()] {
            assert_eq!(
                validate_target(&current, &text(bad)).unwrap_err().kind,
                "invalid"
            );
        }
        let missing = root.path().join("missing");
        assert_eq!(
            validate_target(&current, &text(&missing)).unwrap_err().kind,
            "invalid"
        );
    }
}
