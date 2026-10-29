//! Debug-only performance seed: `seed_fake_library(count)` inserts fake games (no
//! files) with metadata so the virtual grid can be measured at 1 000–2 000 games.
//! Always registered (one static handler list); release builds answer an error.
//! Call from the dev window console:
//! `window.__TAURI_INTERNALS__.invoke("seed_fake_library", { count: 1000 })`.
//! The seed folder does not exist on disk: a rescan reports it as not found and
//! removing it in Settings hides the fake games again.

use tauri::State;

use crate::app_state::AppState;
use crate::command_error::{CommandError, CommandResult};

/// Upper bound per call (keeps an accidental huge number from filling the DB).
#[cfg(debug_assertions)]
const MAX_SEED: u32 = 5_000;

/// Inserts `count` fake games; returns how many were written.
#[tauri::command]
pub fn seed_fake_library(state: State<'_, AppState>, count: u32) -> CommandResult<usize> {
    #[cfg(debug_assertions)]
    {
        seed(&state, count.min(MAX_SEED))
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = (state, count);
        Err(CommandError::invalid(
            "seed_fake_library is available in debug builds only",
        ))
    }
}

#[cfg(debug_assertions)]
fn seed(state: &AppState, count: u32) -> CommandResult<usize> {
    use storage_db::{GameMetadataRecord, ScannedGame, ScannedMetadata};

    const GENRES: [&str; 6] = [
        "Action",
        "Platform",
        "Role-playing (RPG)",
        "Sports",
        "Racing",
        "Puzzle",
    ];
    let folder_path = state.paths.data_root.join("seed-library");
    let folder_path = folder_path.to_string_lossy().into_owned();
    let mut conn = state.db();
    let folder = match storage_db::list_library_folders(&conn)?
        .into_iter()
        .find(|f| f.path == folder_path)
    {
        Some(folder) => folder,
        None => storage_db::add_library_folder(&conn, &folder_path, crate::app_state::unix_now())?,
    };
    // Fake games of the first registry system (perf seeding only).
    let system = system_catalog::ALL_SYSTEMS[0];
    let games: Vec<ScannedGame> = (0..count)
        .map(|i| {
            let title = format!("Seed Game {i:04}");
            ScannedGame {
                platform_id: system.id.to_string(),
                folder_id: folder.id,
                rom_path: format!("{folder_path}/{title}.{}", system.extensions[0]),
                file_size: 1_048_576,
                file_mtime_ns: 0,
                crc32: format!("5EED{i:04X}"),
                sha1: format!("5EED{i:036X}"),
                title: title.clone(),
                region: Some("USA".into()),
                dat_name: (i % 9 != 0).then(|| format!("{title} (USA)")),
                metadata: (i % 9 != 0).then(|| ScannedMetadata {
                    record: GameMetadataRecord {
                        developer: Some(format!("Studio {}", i % 40)),
                        genre: Some(GENRES[i as usize % GENRES.len()].to_string()),
                        release_year: Some(1990 + (i % 10) as u16),
                        players: Some(1 + (i % 2) as u8),
                        ..GameMetadataRecord::default()
                    },
                    source_version: game_library::METADATA_SOURCE_VERSION,
                }),
                serial: None,
            }
        })
        .collect();
    let mut written = 0;
    for chunk in games.chunks(200) {
        let outcome =
            storage_db::upsert_scanned_batch(&mut conn, chunk, crate::app_state::unix_now())
                .map_err(CommandError::from)?;
        written += outcome.inserted + outcome.updated;
    }
    tracing::info!(written, "debug seed library written");
    Ok(written)
}
