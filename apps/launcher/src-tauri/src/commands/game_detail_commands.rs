//! Game Detail page: detail view, favorite, metadata edit/reset, genres, reveal ROM.

use std::path::Path;

use game_library::METADATA_SOURCE_VERSION;
use serde::Serialize;
use storage_db::{ArtworkRecord, GameDetail, UserMetadataEdit};
use tauri::{AppHandle, Manager, State};

use crate::app_state::{unix_now, AppState};
use crate::asset_scope_dirs::with_absolute_art;
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;

/// `GameDetail` (game with absolute art paths, metadata, stats) plus display extras.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetailView {
    #[serde(flatten)]
    pub detail: GameDetail,
    pub rom_file_name: String,
    /// Core that plays the game's system (`None` for a platform outside the registry).
    pub core_name: Option<String>,
    pub core_version: Option<String>,
    /// Lookup results per kind (Detail shows "No artwork found" when both are not-found).
    pub artwork: Vec<ArtworkRecord>,
}

fn not_found() -> CommandError {
    CommandError::invalid("This game is no longer in the library.")
}

/// Builds the view for `game_id` (`None` = unknown id).
fn detail_view(state: &AppState, game_id: i64) -> CommandResult<Option<GameDetailView>> {
    let conn = state.db();
    let Some(mut detail) = storage_db::get_game_detail(&conn, game_id)? else {
        return Ok(None);
    };
    detail.game = with_absolute_art(detail.game, &state.paths.artwork_dir);
    let rom_file_name = Path::new(&detail.game.rom_path).file_name().map_or_else(
        || detail.game.rom_path.clone(),
        |n| n.to_string_lossy().into_owned(),
    );
    let core = system_catalog::core_for_system(&detail.game.platform_id);
    let core_version = match core {
        Some(core) => storage_db::get_installed_core(&conn, core.id)?.and_then(|c| c.version),
        None => None,
    };
    Ok(Some(GameDetailView {
        artwork: storage_db::get_artwork(&conn, game_id)?,
        detail,
        rom_file_name,
        core_name: core.map(|c| format!("{} (libretro)", c.display_name)),
        core_version,
    }))
}

/// Detail of one game; `None` when the id is unknown (the page shows a notice).
#[tauri::command]
pub fn get_game_detail(
    state: State<'_, AppState>,
    game_id: i64,
) -> CommandResult<Option<GameDetailView>> {
    detail_view(&state, game_id)
}

#[tauri::command]
pub fn set_favorite(state: State<'_, AppState>, game_id: i64, favorite: bool) -> CommandResult<()> {
    if storage_db::set_favorite(&state.db(), game_id, favorite)? {
        Ok(())
    } else {
        Err(not_found())
    }
}

/// Saves the user's metadata (validated, capped); the row is frozen until reset.
#[tauri::command]
pub fn update_game_metadata(
    state: State<'_, AppState>,
    game_id: i64,
    edit: UserMetadataEdit,
) -> CommandResult<GameDetailView> {
    if storage_db::get_game(&state.db(), game_id)?.is_none() {
        return Err(not_found());
    }
    storage_db::update_user_metadata(
        &state.db(),
        game_id,
        &edit,
        METADATA_SOURCE_VERSION,
        unix_now(),
    )?;
    detail_view(&state, game_id)?.ok_or_else(not_found)
}

/// "Reset to database values": deletes the row, then re-applies the vendored metadat
/// when the game is No-Intro-matched and known to it (else the fields stay empty).
#[tauri::command]
pub async fn reset_game_metadata(app: AppHandle, game_id: i64) -> CommandResult<GameDetailView> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let game = storage_db::get_game(&state.db(), game_id)?.ok_or_else(not_found)?;
        storage_db::delete_metadata(&state.db(), game_id)?;
        let indexed = game.dat_name.as_deref().and_then(|dat_name| {
            state.catalogs.metadata_for_game(
                &game.platform_id,
                Some(dat_name),
                &game.crc32,
                game.serial.as_deref(),
            )
        });
        if let Some((record, version)) = indexed {
            storage_db::upsert_from_index(&state.db(), game_id, record, version, unix_now())?;
        }
        detail_view(&state, game_id)?.ok_or_else(not_found)
    })
    .await
}

/// Genres of visible games, A–Z (Library filter, Edit dialog suggestions).
#[tauri::command]
pub fn list_genres(state: State<'_, AppState>) -> CommandResult<Vec<String>> {
    Ok(storage_db::list_genres(&state.db())?)
}

/// Shows the ROM in the OS file manager. The path comes from the database only.
#[tauri::command]
pub async fn reveal_rom_in_folder(app: AppHandle, game_id: i64) -> CommandResult<()> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let game = storage_db::get_game(&state.db(), game_id)?.ok_or_else(not_found)?;
        if !Path::new(&game.rom_path).exists() {
            return Err(CommandError::invalid(
                "The ROM file was not found. Rescan your library.",
            ));
        }
        tauri_plugin_opener::reveal_item_in_dir(&game.rom_path)
            .map_err(|err| CommandError::new("invalid", format!("Cannot open the folder: {err}")))
    })
    .await
}
