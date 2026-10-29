//! Detail → Save states → Memory cards: the card files a game's core keeps in the
//! game's save folder (`CoreSpec.memory_cards`: slot 1 = the runner's SRAM file,
//! slot 2 = the core's own file), and importing a card from another emulator into
//! a slot. Refused while that game runs; the previous card is kept as
//! `<file>.before-import-<YYMMDD-HHMMSS>`.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use system_catalog::{core_for_system, MemoryCardSlot};
use tauri::{AppHandle, Manager};

use crate::app_state::{lock, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;
use crate::commands::game_data_target::{ensure_idle, GameDataTarget};
use crate::commands::memory_card_import::{
    card_has_serial, normalize_card, save_names, CARD_BYTES,
};

/// One memory card slot of a game.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCardView {
    /// 1-based slot number.
    pub slot: u8,
    pub file_name: String,
    /// Saves on the card; `None` when there is no readable card (see `unreadable`).
    pub saves: Option<usize>,
    /// A file exists but is not a valid card (wrong size, no `MC` header).
    pub unreadable: bool,
}

/// Result of an import.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCardImport {
    pub saves: usize,
    /// A save on the card carries the game's serial; `None` when the game has none.
    pub serial_found: Option<bool>,
}

/// The game's memory card slots (empty for systems without cards).
#[tauri::command]
pub async fn get_memory_cards(app: AppHandle, game_id: i64) -> CommandResult<Vec<MemoryCardView>> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let target = GameDataTarget::load(&state, game_id)?;
        let dir = target.saves_dir(&state.paths.data_root);
        Ok(card_files(&target.system)
            .into_iter()
            .enumerate()
            .map(|(i, file_name)| {
                let path = dir.as_ref().map(|d| d.join(&file_name));
                let card = path.filter(|p| p.is_file()).map(|p| read_card(&p));
                MemoryCardView {
                    slot: i as u8 + 1,
                    saves: card
                        .as_ref()
                        .and_then(|c| c.as_ref().ok())
                        .map(|c| save_names(c).len()),
                    unreadable: card.is_some_and(|c| c.is_err()),
                    file_name,
                }
            })
            .collect())
    })
    .await
}

/// Imports the card at `path` into `slot` (1-based) of `game_id`.
#[tauri::command]
pub async fn import_memory_card(
    app: AppHandle,
    game_id: i64,
    slot: u8,
    path: String,
) -> CommandResult<MemoryCardImport> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let target = GameDataTarget::load(&state, game_id)?;
        let file_name = usize::from(slot)
            .checked_sub(1)
            .and_then(|i| card_files(&target.system).into_iter().nth(i))
            .ok_or_else(|| CommandError::invalid("This game has no such memory card slot."))?;
        let dir = target.saves_dir(&state.paths.data_root).ok_or_else(|| {
            CommandError::invalid("The ROM file was not found. Rescan your library.")
        })?;
        let card = read_card(Path::new(&path)).map_err(CommandError::invalid)?;
        let serial = storage_db::get_game(&state.db(), game_id)?.and_then(|g| g.serial);
        let session = lock(&state.session);
        ensure_idle(session.map(|s| s.game_id), game_id)?;
        write_card(&dir, &file_name, &card)?;
        drop(session);
        tracing::info!(game_id, slot, file = %file_name, "memory card imported");
        Ok(MemoryCardImport {
            saves: save_names(&card).len(),
            serial_found: serial.map(|s| card_has_serial(&card, &s)),
        })
    })
    .await
}

/// File name per slot, from the system's core (no system literals here).
fn card_files(system: &str) -> Vec<String> {
    core_for_system(system)
        .map(|core| core.memory_cards)
        .unwrap_or_default()
        .iter()
        .map(|slot| match slot {
            MemoryCardSlot::SaveRam => ipc_protocol::SRAM_FILE_NAME.to_owned(),
            MemoryCardSlot::CoreFile(name) => (*name).to_owned(),
        })
        .collect()
}

/// Reads and normalises a card file (size capped just above the largest format).
fn read_card(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(CARD_BYTES as u64 + 4096).read_to_end(&mut bytes))
        .map_err(|err| format!("Cannot read {}: {err}", path.display()))?;
    normalize_card(&bytes)
}

/// Backs up the current card (if any), then writes the new one via a temp file.
fn write_card(dir: &Path, file_name: &str, card: &[u8]) -> CommandResult<()> {
    let storage = |err: std::io::Error| {
        CommandError::new("storage", format!("Cannot write the memory card: {err}"))
    };
    std::fs::create_dir_all(dir).map_err(storage)?;
    let target = dir.join(file_name);
    if target.is_file() {
        let stamp = chrono::Local::now().format("%y%m%d-%H%M%S");
        let backup: PathBuf = dir.join(format!("{file_name}.before-import-{stamp}"));
        std::fs::copy(&target, &backup).map_err(storage)?;
    }
    let partial = dir.join(format!("{file_name}.part"));
    std::fs::write(&partial, card).map_err(storage)?;
    std::fs::rename(&partial, &target).map_err(storage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_follow_the_core_and_writes_keep_a_backup() {
        assert_eq!(card_files("psx"), ["save.srm", "pcsx-card2.mcd"]);
        assert!(card_files("snes").is_empty());
        let dir = tempfile::tempdir().unwrap();
        let mut card = vec![0u8; CARD_BYTES];
        card[..2].copy_from_slice(b"MC");
        write_card(dir.path(), "save.srm", &card).unwrap();
        card[200] = 7;
        write_card(dir.path(), "save.srm", &card).unwrap();
        assert_eq!(std::fs::read(dir.path().join("save.srm")).unwrap(), card);
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names
            .iter()
            .any(|n| n.starts_with("save.srm.before-import-")));
    }
}
