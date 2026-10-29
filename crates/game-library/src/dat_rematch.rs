//! Re-identifies already-scanned "needs review" games after the DAT changed (or was
//! missing at first scan). Uses the stored hashes, so no file is read.

use std::collections::HashMap;
use std::sync::Mutex;

use storage_db::{list_games, upsert_scanned_batch, Connection, GameFilter, GameListQuery};

use crate::folder_scanner::DiscoveredFile;
use crate::library_error::LibraryError;
use crate::rom_file_hasher::RomHashes;
use crate::scanned_game_builder::{scanned_disc_game, scanned_game_from_hashes};
use crate::system_catalogs::SystemCatalogs;
use system_catalog::{hash_skip_len, Media};

/// Looks up `unchanged` files that are stored without a DAT name; rewrites those the DAT
/// now knows (with their metadata). Returns how many were identified. Whole-file
/// hashes only: the NES headerless fallback needs the file bytes (next re-hash).
/// Discs: stored track SHA1, then stored serial (their `file.size` sums every file
/// of the game, so no CRC + size lookup). Cartridges: hashes, then the stored
/// header serial (DS game code).
pub(crate) fn rematch_unidentified(
    db: &Mutex<Connection>,
    folder_id: i64,
    unchanged: &[&DiscoveredFile],
    catalogs: &SystemCatalogs,
    now: i64,
) -> Result<usize, LibraryError> {
    if unchanged.is_empty() {
        return Ok(0);
    }
    let query = GameListQuery {
        filter: GameFilter::NeedsReview,
        ..GameListQuery::default()
    };
    let stored = list_games(&crate::library_scan_job::lock(db), &query)?;
    let by_path: HashMap<&str, _> = stored
        .iter()
        .filter(|game| game.folder_id == Some(folder_id) && !game.missing)
        .map(|game| (game.rom_path.as_str(), game))
        .collect();

    let mut identified = Vec::new();
    for file in unchanged {
        let Some(game) = by_path.get(file.path.as_str()) else {
            continue;
        };
        let catalog = catalogs.get(file.system.id);
        if catalog.dat.is_empty() {
            continue;
        }
        if file.system.media == Media::Disc {
            let known = catalog.dat.lookup_sha1(&game.sha1).is_some()
                || game
                    .serial
                    .as_deref()
                    .is_some_and(|serial| catalog.dat.lookup_serial(serial, None).is_some());
            if known {
                let hashes = RomHashes {
                    crc32: game.crc32.clone(),
                    sha1: game.sha1.clone(),
                    size: 0, // unknown here: no CRC + size match, no size tie-break
                };
                let serial = game.serial.clone();
                // No file I/O here: an eboot keeps the title its scan read from it.
                let own_title = is_pbp(&file.path).then(|| game.title.clone());
                identified.push(scanned_disc_game(
                    file, folder_id, catalog, hashes, serial, own_title,
                ));
            }
            continue;
        }
        let header = hash_skip_len(file.system.header_rule, file.size) as u64;
        let hashes = RomHashes {
            crc32: game.crc32.clone(),
            sha1: game.sha1.clone(),
            size: file.size.saturating_sub(header),
        };
        let serial = game.serial.clone();
        let by_serial = |code: &str| catalog.dat.lookup_serial(code, None).is_some();
        if catalog.dat.lookup(&hashes).is_some() || serial.as_deref().is_some_and(by_serial) {
            identified.push(scanned_game_from_hashes(
                file, folder_id, catalog, hashes, serial,
            ));
        }
    }
    let count = identified.len();
    if count > 0 {
        upsert_scanned_batch(&mut crate::library_scan_job::lock(db), &identified, now)?;
    }
    Ok(count)
}

fn is_pbp(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(disc_image::PBP_EXT))
}
