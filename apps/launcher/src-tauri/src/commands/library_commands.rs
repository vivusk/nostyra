//! Setup state, library folders and game list queries.

use std::path::Path;

use serde::Serialize;
use storage_db::{Game, GameListQuery, LibraryFolder, LibrarySummary};
use tauri::State;

use crate::app_state::{lock, unix_now, AppState};
use crate::asset_scope_dirs::with_absolute_art;
use crate::command_error::{CommandError, CommandResult};

/// What the shell needs on start: first-run or library, plus running jobs.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupState {
    pub has_folders: bool,
    /// Every registry core and whether it is installed (none is required to finish
    /// setup: a missing core is offered on first Play of its system).
    pub cores: Vec<CoreSetupStatus>,
    /// Bundled DAT missing/corrupt → every game "needs review".
    pub dat_missing: bool,
    /// Shown read-only in Settings.
    pub data_root: String,
    pub scanning: bool,
    pub artwork_fetching: bool,
    pub core_downloading: bool,
    pub active_game_id: Option<i64>,
}

/// One registry core in the setup state.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSetupStatus {
    pub id: &'static str,
    pub display_name: &'static str,
    pub installed: bool,
    /// System ids it plays.
    pub systems: &'static [&'static str],
}

fn core_setup_statuses(cores_dir: &Path) -> Vec<CoreSetupStatus> {
    system_catalog::CORES
        .iter()
        .map(|core| CoreSetupStatus {
            id: core.id,
            display_name: core.display_name,
            installed: core_manager::installed_core(cores_dir, core.id).is_some(),
            systems: core.systems,
        })
        .collect()
}

#[tauri::command]
pub fn get_setup_state(state: State<'_, AppState>) -> CommandResult<SetupState> {
    let has_folders = !storage_db::list_library_folders(&state.db())?.is_empty();
    Ok(SetupState {
        has_folders,
        cores: core_setup_statuses(&state.paths.cores_dir),
        dat_missing: state.catalogs.any_dat_missing(),
        data_root: state.paths.data_root.to_string_lossy().into_owned(),
        scanning: lock(&state.scan_cancel).is_some(),
        artwork_fetching: lock(&state.artwork_cancel).is_some(),
        core_downloading: lock(&state.core_download).is_some(),
        active_game_id: lock(&state.session).map(|s| s.game_id),
    })
}

#[tauri::command]
pub fn list_library_folders(state: State<'_, AppState>) -> CommandResult<Vec<LibraryFolder>> {
    Ok(storage_db::list_library_folders(&state.db())?)
}

/// Adds a folder picked in the OS dialog (validated: must exist, UTF-8 path).
#[tauri::command]
pub fn add_library_folder(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<LibraryFolder> {
    if !Path::new(&path).is_dir() {
        return Err(CommandError::invalid(format!("Folder not found: {path}")));
    }
    Ok(storage_db::add_library_folder(
        &state.db(),
        &path,
        unix_now(),
    )?)
}

/// Removes a folder: its games are hidden, play history is kept for a re-add.
#[tauri::command]
pub fn remove_library_folder(state: State<'_, AppState>, id: i64) -> CommandResult<bool> {
    // The scan holds a snapshot of the folder list; removing one under it would make
    // its writes fail. Held for the removal so no scan starts in between.
    let scan = lock(&state.scan_cancel);
    if scan.is_some() {
        return Err(CommandError::new(
            "scan",
            "Wait for the scan to finish (or cancel it) before removing a folder.",
        ));
    }
    Ok(storage_db::remove_library_folder(&mut state.db(), id)?)
}

/// Library list; cover/hero paths are absolute (ready for `convertFileSrc`).
#[tauri::command]
pub fn list_games(state: State<'_, AppState>, query: GameListQuery) -> CommandResult<Vec<Game>> {
    let games = storage_db::list_games(&state.db(), &query)?;
    let dir = &state.paths.artwork_dir;
    Ok(games
        .into_iter()
        .map(|g| with_absolute_art(g, dir))
        .collect())
}

#[tauri::command]
pub fn library_summary(state: State<'_, AppState>) -> CommandResult<LibrarySummary> {
    Ok(storage_db::library_summary(&state.db())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registry_core_is_listed_and_none_is_required() {
        let empty = tempfile::Builder::new()
            .prefix("nostyra-no-cores-")
            .tempdir()
            .unwrap();
        let cores = core_setup_statuses(empty.path());
        assert_eq!(cores.len(), system_catalog::CORES.len());
        assert!(cores.iter().all(|core| !core.installed));
    }
}
