//! Turns one discovered file into a storage-db row: hash, DAT match, display title.

use std::path::Path;

use storage_db::{GameMetadataRecord, ScannedGame, ScannedMetadata};

use crate::dat_index::{display_region, display_title};
use crate::disc_game_identity::read_disc_identity;
use crate::folder_scanner::{DiscoveredFile, ScanFailure};
use crate::nds_game_code::cartridge_serial;
use crate::rom_file_hasher::{hash_headerless, hash_rom_file, RomHashes};
use crate::serial_match_keys::serial_rule;
use crate::system_catalogs::SystemCatalog;
use crate::DatEntry;

/// Hashes `file` and identifies it. Unknown dumps get a title from the file name
/// and no DAT name ("needs review"); matched dumps also get their vendored metadata
/// (keyed by the DAT entry's `metadata_crc32`). NES: a DAT miss on the whole-file
/// hash retries with the headerless hash (the DAT's `.unh` entries), so re-headered
/// dumps still match; the stored hashes stay whole-file (stable relink key). A
/// cartridge with a header serial (DS game code) is stored with it and, on a hash
/// miss, matched by it (see [`scanned_game_from_match`]). Discs: see [`scanned_disc_game`].
pub fn build_scanned_game(
    file: &DiscoveredFile,
    folder_id: i64,
    catalog: &SystemCatalog,
) -> Result<ScannedGame, ScanFailure> {
    let path = Path::new(&file.path);
    let failure = |reason: String| ScanFailure {
        path: path.to_path_buf(),
        reason,
    };
    if let Some(signature) = serial_rule(file.system) {
        let identity =
            read_disc_identity(path, signature).map_err(|err| failure(err.to_string()))?;
        return Ok(scanned_disc_game(
            file,
            folder_id,
            catalog,
            identity.hashes,
            identity.serial,
            identity.title,
        ));
    }
    let (hashes, bytes) =
        hash_rom_file(path, file.system).map_err(|err| failure(err.to_string()))?;
    let matched = catalog.dat.lookup(&hashes).or_else(|| {
        hash_headerless(&bytes, file.system).and_then(|bare| catalog.dat.lookup(&bare))
    });
    let serial = cartridge_serial(path, file.system);
    Ok(scanned_game_from_match(
        file, folder_id, catalog, hashes, matched, serial,
    ))
}

/// Same as [`build_scanned_game`] when the hashes (and the stored header serial)
/// are already known (no file I/O, so whole-file lookup only).
pub(crate) fn scanned_game_from_hashes(
    file: &DiscoveredFile,
    folder_id: i64,
    catalog: &SystemCatalog,
    hashes: RomHashes,
    serial: Option<String>,
) -> ScannedGame {
    let matched = catalog.dat.lookup(&hashes);
    scanned_game_from_match(file, folder_id, catalog, hashes, matched, serial)
}

/// Disc row: the identity track's hash matches the DAT → DAT title; else the boot
/// serial matches ("probable": DAT name, region, artwork and metadata, but the title
/// stays the file name — translations and hacks share their original's serial —
/// or `own_title`, the name a PSP eboot gives itself). Metadata by serial: the
/// disc's, else the matched entry's. GameCube / Wii: the hash is the canonical
/// game-ID text's (`gamecube_wii_identity`), so only the serial ever matches.
pub(crate) fn scanned_disc_game(
    file: &DiscoveredFile,
    folder_id: i64,
    catalog: &SystemCatalog,
    hashes: RomHashes,
    serial: Option<String>,
    own_title: Option<String>,
) -> ScannedGame {
    let by_hash = catalog.dat.lookup(&hashes);
    let matched = by_hash.or_else(|| {
        let size = (hashes.size > 0).then_some(hashes.size);
        serial
            .as_deref()
            .and_then(|serial| catalog.dat.lookup_serial(serial, size))
    });
    let serial = serial.or_else(|| matched.and_then(|entry| entry.serial.clone()));
    let record = serial
        .as_deref()
        .and_then(|s| catalog.metadata.lookup_serial(s))
        .or_else(|| {
            let entry_serial = matched.and_then(|entry| entry.serial.as_deref())?;
            catalog.metadata.lookup_serial(entry_serial)
        });
    let title = match by_hash {
        Some(entry) => display_title(&entry.name),
        None => own_title.unwrap_or_else(|| stem_title(file)),
    };
    let mut game = row(file, folder_id, catalog, hashes, matched, record, title);
    game.serial = serial;
    game
}

/// Cartridge row: `by_hash` → DAT title; else the header `serial` matches the DAT
/// (size tie-break; "probable" like a disc: DAT name, region, artwork and metadata,
/// title stays the file name). The serial is stored either way.
fn scanned_game_from_match(
    file: &DiscoveredFile,
    folder_id: i64,
    catalog: &SystemCatalog,
    hashes: RomHashes,
    by_hash: Option<&DatEntry>,
    serial: Option<String>,
) -> ScannedGame {
    let matched = by_hash.or_else(|| {
        let size = Some(hashes.size);
        serial
            .as_deref()
            .and_then(|code| catalog.dat.lookup_serial(code, size))
    });
    let record = matched.and_then(|entry| catalog.metadata.lookup(&entry.metadata_crc32));
    let title = by_hash.map_or_else(|| stem_title(file), |entry| display_title(&entry.name));
    let mut game = row(file, folder_id, catalog, hashes, matched, record, title);
    game.serial = serial;
    game
}

/// Card title from the file name ("Game (U) [!]" → "Game").
fn stem_title(file: &DiscoveredFile) -> String {
    let stem = Path::new(&file.path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&file.path);
    display_title(stem)
}

fn row(
    file: &DiscoveredFile,
    folder_id: i64,
    catalog: &SystemCatalog,
    hashes: RomHashes,
    matched: Option<&DatEntry>,
    record: Option<&GameMetadataRecord>,
    title: String,
) -> ScannedGame {
    ScannedGame {
        platform_id: file.system.id.to_string(),
        folder_id,
        rom_path: file.path.clone(),
        file_size: i64::try_from(file.size).unwrap_or(i64::MAX),
        file_mtime_ns: file.mtime_ns,
        crc32: hashes.crc32,
        sha1: hashes.sha1,
        title,
        region: matched.and_then(display_region),
        dat_name: matched.map(|entry| entry.name.clone()),
        metadata: record.map(|record| ScannedMetadata {
            record: record.clone(),
            source_version: catalog.metadata.source_version(),
        }),
        serial: None,
    }
}
