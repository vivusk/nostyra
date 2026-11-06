//! Turns one discovered file into a storage-db row: hash, DAT match, display title.

use std::path::Path;

use storage_db::{ScannedGame, ScannedMetadata};

use rom_identity::{
    dat_facts, game_identity, match_dat, scanned_game_metadata, GameIdentity, IdentityError,
};

use crate::folder_scanner::{DiscoveredFile, ScanFailure};
use crate::rom_file_hasher::RomHashes;
use crate::system_catalogs::{storage_record, SystemCatalog};

/// Hashes `file` and identifies it. Unknown dumps get a title from the file name
/// and no DAT name ("needs review"); matched dumps also get their vendored metadata
/// (keyed by the DAT entry's `metadata_crc32`). NES: a DAT miss on the whole-file
/// hash retries with the headerless hash (the DAT's `.unh` entries), so re-headered
/// dumps still match; the stored hashes stay whole-file (stable relink key). A
/// cartridge with a header serial (DS game code) is stored with it and, on a hash
/// miss, matched by it (`rom_identity::match_dat`). Discs: see [`scanned_disc_game`].
pub fn build_scanned_game(
    file: &DiscoveredFile,
    folder_id: i64,
    catalog: &SystemCatalog,
) -> Result<ScannedGame, ScanFailure> {
    let path = Path::new(&file.path);
    let identity = game_identity(path, file.system).map_err(|err| ScanFailure {
        path: path.to_path_buf(),
        reason: match err {
            IdentityError::ZipSystemMismatch => {
                "ROM system in ZIP changed since discovery; rescan the library".into()
            }
            IdentityError::Read(message) => message,
        },
    })?;
    Ok(scanned_game_from_identity(
        file, folder_id, catalog, identity,
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
    let identity = GameIdentity {
        hashes,
        headerless: None,
        serial,
        own_title: None,
        disc: false,
    };
    scanned_game_from_identity(file, folder_id, catalog, identity)
}

/// Disc row: the identity track's hash matches the DAT → DAT title; else the boot
/// serial matches ("probable": DAT name, region, artwork and metadata, but the title
/// stays the file name or `own_title`, see `rom_identity::dat_facts`). Metadata by
/// serial: the disc's, else the matched entry's. GameCube / Wii: the hash is the
/// canonical game-ID text's (`gamecube_wii_identity`), so only the serial ever matches.
pub(crate) fn scanned_disc_game(
    file: &DiscoveredFile,
    folder_id: i64,
    catalog: &SystemCatalog,
    hashes: RomHashes,
    serial: Option<String>,
    own_title: Option<String>,
) -> ScannedGame {
    let identity = GameIdentity {
        hashes,
        headerless: None,
        serial,
        own_title,
        disc: true,
    };
    scanned_game_from_identity(file, folder_id, catalog, identity)
}

/// One row from an identity: DAT match + row facts + metadata (`rom_identity` rules).
fn scanned_game_from_identity(
    file: &DiscoveredFile,
    folder_id: i64,
    catalog: &SystemCatalog,
    identity: GameIdentity,
) -> ScannedGame {
    let matched = match_dat(&catalog.dat, &identity);
    let facts = dat_facts(&identity, matched, &file_stem(file));
    let record = scanned_game_metadata(&catalog.metadata, &identity, matched, &facts);
    ScannedGame {
        platform_id: file.system.id.to_string(),
        folder_id,
        rom_path: file.path.clone(),
        file_size: i64::try_from(file.size).unwrap_or(i64::MAX),
        file_mtime_ns: file.mtime_ns,
        crc32: identity.hashes.crc32,
        sha1: identity.hashes.sha1,
        title: facts.title,
        region: facts.region,
        dat_name: facts.dat_name,
        metadata: record.map(|record| ScannedMetadata {
            record: storage_record(record),
            source_version: catalog.metadata.source_version(),
        }),
        serial: facts.serial,
    }
}

/// The file name without extension (fallback title source).
fn file_stem(file: &DiscoveredFile) -> String {
    Path::new(&file.path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&file.path)
        .to_string()
}
