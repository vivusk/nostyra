//! Removing games whose ROM file is missing, for good: the DB row (play history,
//! metadata, favorite, artwork rows cascade) plus the files the app keeps for it —
//! save folder (SRAM, save states, resume point — the web edition's `web-*` states
//! and `save.web.srm` in it belong to the game too), game config layer and cached art.
//! Only entries carrying the game's content id are deleted; a legacy `<rom key>`
//! entry (maybe shared by several ROMs of that file name) is never touched.
//! A present game is never removed (the Library "Missing" filter lists candidates).

use std::io;
use std::path::{Path, PathBuf};

use storage_db::{ArtworkRecord, Connection, Game};
use tauri::{AppHandle, Manager};

use crate::app_state::{lock, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;
use crate::commands::game_data_target::ensure_idle;

/// Removes one missing game; `invalid` when the id is unknown or the ROM is present.
#[tauri::command]
pub async fn remove_missing_game(app: AppHandle, game_id: i64) -> CommandResult<()> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        remove_games(&state, &[game_id], true).map(|_| ())
    })
    .await
}

/// Removes every visible missing game; returns how many were removed.
#[tauri::command]
pub async fn remove_all_missing_games(app: AppHandle) -> CommandResult<usize> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let ids = storage_db::visible_missing_game_ids(&state.db())?;
        remove_games(&state, &ids, false)
    })
    .await
}

/// Files first, then the row: a failed file delete keeps the row so the user can
/// retry. Holds the scan lock (a rescan could restore or take over a row mid-way)
/// and the session lock (Play cannot start a game being removed).
fn remove_games(state: &AppState, ids: &[i64], strict: bool) -> CommandResult<usize> {
    let scan = lock(&state.scan_cancel);
    if scan.is_some() {
        return Err(CommandError::new(
            "scan",
            "Wait for the scan to finish (or cancel it) before removing games.",
        ));
    }
    let session = lock(&state.session);
    let mut removed = 0;
    for (index, &game_id) in ids.iter().enumerate() {
        let plan = {
            let conn = state.db();
            match storage_db::get_game(&conn, game_id)? {
                Some(game) if game.missing => RemovalPlan::load(&conn, game, &ids[index..])?,
                _ if strict => {
                    return Err(CommandError::invalid(
                        "Only games whose ROM file is missing can be removed.",
                    ))
                }
                _ => continue,
            }
        };
        ensure_idle(session.map(|s| s.game_id), game_id)?;
        plan.delete_files(&state.paths.data_root, &state.paths.artwork_dir)
            .map_err(|err| {
                CommandError::new("storage", format!("Cannot delete the game's data: {err}"))
            })?;
        if storage_db::delete_missing_game(&state.db(), game_id)? {
            removed += 1;
            tracing::info!(game_id, rom = %plan.rom_path.display(), "missing game removed");
        }
    }
    drop((session, scan));
    Ok(removed)
}

/// What one removal deletes on disk.
#[derive(Debug)]
struct RemovalPlan {
    system: String,
    rom_path: PathBuf,
    /// `games.sha1`: the id in the save folder / game config names.
    content_sha1: String,
    /// Cached art, relative to the artwork dir.
    artwork_files: Vec<String>,
    /// Another row has the same content id (a copy of the same game): those
    /// files are kept.
    key_shared: bool,
}

impl RemovalPlan {
    /// `pending` = this game and the ones still to be removed in the same batch:
    /// they do not count as sharing its files.
    fn load(conn: &Connection, game: Game, pending: &[i64]) -> CommandResult<Self> {
        let id = ipc_protocol::content_id(&game.sha1);
        let key_shared = storage_db::game_files_keys(conn, Some(&game.platform_id))?
            .iter()
            .any(|other| {
                !pending.contains(&other.game_id) && ipc_protocol::content_id(&other.sha1) == id
            });
        let artwork_files = storage_db::get_artwork(conn, game.id)?
            .into_iter()
            .filter_map(|ArtworkRecord { local_path, .. }| local_path)
            .collect();
        Ok(Self {
            system: game.platform_id,
            rom_path: PathBuf::from(&game.rom_path),
            content_sha1: game.sha1,
            artwork_files,
            key_shared,
        })
    }

    fn delete_files(&self, data_root: &Path, artwork_dir: &Path) -> io::Result<()> {
        for file in &self.artwork_files {
            remove_file_if_present(&artwork_dir.join(file))?;
        }
        let Some(id) = ipc_protocol::content_id(&self.content_sha1) else {
            return Ok(());
        };
        if self.key_shared {
            return Ok(());
        }
        let saves = ipc_protocol::saves_system_dir(data_root, &self.system);
        if let Some(dir) = ipc_protocol::find_game_entry(&saves, &id, "") {
            match std::fs::remove_dir_all(&dir) {
                Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
                _ => {}
            }
        }
        let configs = ipc_protocol::game_config_system_dir(data_root, &self.system);
        let ext = ipc_protocol::GAME_CONFIG_EXT;
        match ipc_protocol::find_game_entry(&configs, &id, ext) {
            Some(file) => remove_file_if_present(&file),
            None => Ok(()),
        }
    }
}

fn remove_file_if_present(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "missing_game_removal_commands_tests.rs"]
mod tests;
