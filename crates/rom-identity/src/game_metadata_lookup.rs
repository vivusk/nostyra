//! Which vendored metadat record a game gets. One rule set for the desktop scanner,
//! its self-heal / "Reset to database values", and the web edition.

use crate::dat_facts::DatFacts;
use crate::dat_index::DatIndex;
use crate::game_identity::{DatMatch, GameIdentity};
use crate::metadata_dat_index::MetadataIndex;
use crate::metadata_record::MetadataRecord;

/// Metadata of a freshly identified game: discs by serial (theirs, else the matched
/// entry's), cartridges by the matched entry's metadata CRC.
pub fn scanned_game_metadata<'m>(
    metadata: &'m MetadataIndex,
    identity: &GameIdentity,
    matched: DatMatch<'_>,
    facts: &DatFacts,
) -> Option<&'m MetadataRecord> {
    if identity.disc {
        facts
            .serial
            .as_deref()
            .and_then(|s| metadata.lookup_serial(s))
            .or_else(|| {
                let entry_serial = matched.matched.and_then(|e| e.serial.as_deref())?;
                metadata.lookup_serial(entry_serial)
            })
    } else {
        matched
            .matched
            .and_then(|entry| metadata.lookup(&entry.metadata_crc32))
    }
}

/// Metadata of a stored game. Looked up by the stored `crc32`, else (re-headered NES
/// rows store a CRC the metadat does not know) by the metadata CRC of the DAT game
/// `dat_name`, else (disc games, metadat keyed by serial) by the stored `serial`,
/// else the DAT game's serial.
pub fn stored_game_metadata<'m>(
    dat: &DatIndex,
    metadata: &'m MetadataIndex,
    dat_name: Option<&str>,
    crc32: &str,
    serial: Option<&str>,
) -> Option<&'m MetadataRecord> {
    let by_name = dat_name.and_then(|name| dat.metadata_crc_for_name(name));
    let dat_serial = dat_name.and_then(|name| dat.serial_for_name(name));
    metadata
        .lookup(crc32)
        .or_else(|| by_name.and_then(|crc| metadata.lookup(crc)))
        .or_else(|| serial.and_then(|s| metadata.lookup_serial(s)))
        .or_else(|| dat_serial.and_then(|s| metadata.lookup_serial(s)))
}
