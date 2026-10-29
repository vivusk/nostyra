//! Save-state slots of one game, read straight from its save folder (the files
//! are the source of truth; no DB table): list with sidecar info and thumbnail
//! (the `auto` resume point first, then slots `0..=9`), and delete. Loading a slot
//! is `play_game` with `load_state`; in achievement hardcore every slot is listed
//! but marked not loadable (`achievements_mode`).

use std::ffi::OsString;
use std::path::Path;
use std::time::UNIX_EPOCH;

use ipc_protocol::{
    state_file, StateSidecar, StateSlotId, MAX_STATE_SLOT, SIDECAR_FILE_EXT, STATE_FILE_EXT,
    STATE_SLOT_FILE_EXTS, THUMBNAIL_FILE_EXT,
};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::app_state::{lock, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::achievements_mode::{block_loads_in_hardcore, hardcore_for_game};
use crate::commands::blocking::run_blocking;
use crate::commands::game_data_target::{ensure_idle, GameDataTarget};
use crate::commands::state_load_rule::StateLoadRule;

/// One used slot (`slotN.state` or `auto.state` exists).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveStateSlot {
    /// `0..=9`, or `"auto"` for the resume point.
    pub slot: StateSlotId,
    /// Unix seconds: the sidecar's `created_at`, else the state file's mtime. Also
    /// the thumbnail URL's cache-buster (a re-save rewrites the PNG in place).
    pub created_at: i64,
    /// `None` when the sidecar is missing or unreadable.
    pub core_name: Option<String>,
    pub core_version: Option<String>,
    /// Absolute `slotN.png` / `auto.png` path (served via the asset protocol), if written.
    pub thumbnail_path: Option<String>,
    pub size_bytes: u64,
    /// Why the runner would refuse this state ("Made by another Dolphin version",
    /// "Made from another disc or revision"); `None` = loadable.
    pub incompatible_reason: Option<String>,
}

/// Used slots of `game_id`: the resume point first, then numbered slots lowest
/// first. A game without a usable ROM path has none.
#[tauri::command]
pub async fn list_save_states(app: AppHandle, game_id: i64) -> CommandResult<Vec<SaveStateSlot>> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let target = GameDataTarget::load(&state, game_id)?;
        let Some(dir) = target.saves_dir(&state.paths.data_root) else {
            return Ok(Vec::new());
        };
        let mut slots = list_slots(&dir, &StateLoadRule::for_game(&state, &target)?);
        if hardcore_for_game(&state.paths.data_root, &target) {
            block_loads_in_hardcore(&mut slots);
        }
        Ok(slots)
    })
    .await
}

/// Removes `slotN.*` or `auto.*` (state, sidecar, thumbnail). Refused (`busy`) while
/// this game runs: the session lock is held throughout, so Play cannot start
/// mid-delete.
#[tauri::command]
pub async fn delete_save_state(
    app: AppHandle,
    game_id: i64,
    slot: StateSlotId,
) -> CommandResult<()> {
    run_blocking(move || {
        let slot = validated(slot)?;
        let state = app.state::<AppState>();
        let target = GameDataTarget::load(&state, game_id)?;
        let Some(dir) = target.saves_dir(&state.paths.data_root) else {
            return Err(CommandError::invalid(
                "The ROM file was not found. Rescan your library.",
            ));
        };
        let session = lock(&state.session);
        ensure_idle(session.map(|s| s.game_id), game_id)?;
        delete_slot_files(&dir, slot).map_err(|err| {
            CommandError::new("storage", format!("Cannot delete the save: {err}"))
        })?;
        drop(session);
        tracing::info!(game_id, %slot, "save state deleted");
        Ok(())
    })
    .await
}

/// Runner arguments that load `slot` at start (`play_game`: `--load-state 3|auto`),
/// after validating it; empty without a slot.
pub fn load_state_args(slot: Option<StateSlotId>) -> CommandResult<Vec<OsString>> {
    let Some(slot) = slot else {
        return Ok(Vec::new());
    };
    let slot = validated(slot)?;
    Ok(vec!["--load-state".into(), slot.to_string().into()])
}

/// The slot after a round trip through its validated spelling, so an
/// out-of-range `Numbered` can never reach a file name or the runner.
fn validated(slot: StateSlotId) -> CommandResult<StateSlotId> {
    slot.to_string().parse().map_err(|_| {
        CommandError::invalid(format!(
            "Save slot {slot} does not exist (0–{MAX_STATE_SLOT} or auto)."
        ))
    })
}

/// Every slot in `dir` that holds a state file, resume point first, each checked
/// against `rule`; other files are ignored.
fn list_slots(dir: &Path, rule: &StateLoadRule) -> Vec<SaveStateSlot> {
    std::iter::once(StateSlotId::Auto)
        .chain((0..=MAX_STATE_SLOT).map(StateSlotId::Numbered))
        .filter_map(|slot| read_slot(dir, slot, rule))
        .collect()
}

fn read_slot(dir: &Path, slot: StateSlotId, rule: &StateLoadRule) -> Option<SaveStateSlot> {
    let meta = std::fs::metadata(state_file(dir, slot, STATE_FILE_EXT)).ok()?;
    if !meta.is_file() {
        return None;
    }
    let sidecar = std::fs::read(state_file(dir, slot, SIDECAR_FILE_EXT))
        .ok()
        .and_then(|raw| serde_json::from_slice::<StateSidecar>(&raw).ok());
    let mtime = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_secs());
    let created_at = sidecar.as_ref().map_or(mtime, |s| s.created_at);
    let thumbnail = state_file(dir, slot, THUMBNAIL_FILE_EXT);
    Some(SaveStateSlot {
        slot,
        incompatible_reason: sidecar.as_ref().and_then(|s| rule.incompatible_reason(s)),
        created_at: i64::try_from(created_at).unwrap_or(i64::MAX),
        core_name: sidecar.as_ref().map(|s| s.core_name.clone()),
        core_version: sidecar.map(|s| s.core_version),
        thumbnail_path: thumbnail
            .is_file()
            .then(|| thumbnail.to_string_lossy().into_owned()),
        size_bytes: meta.len(),
    })
}

/// Deletes the slot's files that exist. State first: once it is gone the slot no
/// longer lists, so a later failure only leaves orphan files a re-save overwrites.
fn delete_slot_files(dir: &Path, slot: StateSlotId) -> std::io::Result<()> {
    for ext in STATE_SLOT_FILE_EXTS {
        match std::fs::remove_file(state_file(dir, slot, ext)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "save_state_commands_tests.rs"]
mod tests;
