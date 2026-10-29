//! Startup catalog warm-up + metadata self-heal. Scans only write metadata for
//! new/changed files, so libraries scanned before M2 (or with an older metadat
//! version) are completed here on every start: missing rows always, stale non-edited
//! rows when the setting `metadata_source_version` differs from the vendored version.
//! Idempotent. A partial index (missing/corrupt file) stamps rows and the setting with
//! the "+partial" version, so the next full load re-heals everything it wrote.

use std::time::Instant;

use game_library::{METADATA_PARTIAL_VERSION, METADATA_SOURCE_VERSION};
use storage_db::{Connection, GameMetadataRecord, MetadataWorkItem, StorageResult};
use tauri::{AppHandle, Manager};

use crate::app_state::{unix_now, AppState};

/// Setting holding the metadat version of the last self-heal (`+partial` when the index
/// was incomplete).
pub const METADATA_VERSION_SETTING: &str = "metadata_source_version";

/// Off the UI thread: loads the catalogs of every system that already has games (so
/// the first scan and the self-heal do not wait), then heals metadata rows.
pub fn spawn_catalog_warmup(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let started = Instant::now();
        let platforms = storage_db::game_platform_ids(&state.db()).unwrap_or_else(|err| {
            tracing::warn!(%err, "library systems not listed; self-heal skipped");
            Vec::new()
        });
        if platforms.is_empty() {
            // No games (or unknown systems): no catalog was checked, so nothing may be
            // stamped complete; the next start heals once games exist.
            return;
        }
        for platform_id in &platforms {
            state.catalogs.get(platform_id);
        }
        tracing::info!(
            ?platforms,
            ms = started.elapsed().as_millis(),
            "catalog warm-up done"
        );
        let catalogs = &state.catalogs;
        let lookup = |work: &MetadataWorkItem| {
            catalogs
                .metadata_for_game(
                    &work.platform_id,
                    work.dat_name.as_deref(),
                    &work.crc32,
                    work.serial.as_deref(),
                )
                .map(|(record, _)| record.clone())
        };
        let complete = catalogs.loaded_complete();
        let result = heal_metadata(&mut state.db(), &lookup, complete, unix_now());
        match result {
            Ok(healed) => tracing::info!(healed, "metadata self-heal done"),
            Err(err) => tracing::warn!(%err, "metadata self-heal failed"),
        }
    });
}

/// Applies index metadata to every game that needs it; returns rows written.
/// `lookup` resolves a work item in its system's catalog. `complete` = every loaded
/// catalog's metadat loaded fully: only then are rows stamped with (and the setting
/// set to) the real version and stale rows absent from it removed; a partial index
/// stamps the "+partial" version so the next full load re-heals.
pub fn heal_metadata(
    conn: &mut Connection,
    lookup: &dyn Fn(&MetadataWorkItem) -> Option<GameMetadataRecord>,
    complete: bool,
    now: i64,
) -> StorageResult<usize> {
    let recorded = storage_db::get_setting(conn, METADATA_VERSION_SETTING)?;
    let stale = recorded.as_deref() != Some(METADATA_SOURCE_VERSION);
    let work = storage_db::games_needing_metadata(conn, METADATA_SOURCE_VERSION, stale)?;
    let version = if complete {
        METADATA_SOURCE_VERSION
    } else {
        METADATA_PARTIAL_VERSION
    };
    let tx = conn.transaction()?;
    let mut written = 0;
    for item in work {
        match lookup(&item) {
            Some(record) => {
                storage_db::upsert_from_index(&tx, item.game_id, &record, version, now)?;
                written += 1;
            }
            // The work list holds only non-edited or absent rows: safe to drop.
            None if complete => storage_db::delete_metadata(&tx, item.game_id)?,
            None => {}
        }
    }
    storage_db::set_setting(&tx, METADATA_VERSION_SETTING, version)?;
    tx.commit()?;
    Ok(written)
}

#[cfg(test)]
#[path = "metadata_self_heal_tests.rs"]
mod tests;
