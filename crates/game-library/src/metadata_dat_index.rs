//! Offline game metadata from the vendored libretro-database `metadat/` attribute DATs
//! (one folder per attribute), merged into two lookups: by ROM CRC32 (No-Intro
//! systems: each file carries its own attribute) and by game `serial` (PlayStation:
//! one serial-keyed file carries every attribute, ROMs have no CRC).

use std::collections::HashMap;
use std::path::Path;

use storage_db::GameMetadataRecord;
use system_catalog::DiscSignature;

use crate::clrmamepro_dat_parser::{parse_clrmamepro_games, DatGame};
use crate::serial_match_keys::serial_keys;

/// libretro-database commit of the vendored metadat files (`SOURCE.txt`; a test keeps
/// them in sync). One literal feeds both version consts below.
macro_rules! metadat_commit {
    () => {
        "9a899f5b13ed1c355491e342203be29bc573323b"
    };
}

/// Version stamped on metadata rows written from the full index. A new value makes the
/// startup self-heal re-apply non-edited rows.
pub const METADATA_SOURCE_VERSION: &str = metadat_commit!();

/// Version stamped on rows written from an incomplete index (a file was missing or
/// corrupt): differs from [`METADATA_SOURCE_VERSION`], so the next full load re-heals them.
pub const METADATA_PARTIAL_VERSION: &str = concat!(metadat_commit!(), "+partial");

/// Attribute folders under `metadat/` and the key each file uses inside game blocks.
const ATTRIBUTE_FILES: [(&str, &str); 5] = [
    ("developer", "developer"),
    ("publisher", "publisher"),
    ("genre", "genre"),
    ("releaseyear", "releaseyear"),
    ("maxusers", "users"),
];

/// CRC32 (upper-case hex) / serial → merged metadata.
#[derive(Debug, Default, Clone)]
pub struct MetadataIndex {
    by_crc: HashMap<String, GameMetadataRecord>,
    by_serial: HashMap<String, GameMetadataRecord>,
    /// Disc systems: serials also match under the signature's normalised keys.
    serial_rule: Option<DiscSignature>,
    /// False when any attribute file was missing or corrupt (scans and self-heal then
    /// stamp the "+partial" version, so the next full load re-heals).
    complete: bool,
}

impl MetadataIndex {
    /// No metadata: scans still work, games just have no developer/genre/….
    pub fn empty() -> Self {
        Self::default()
    }

    /// Loads the five attribute files `metadat_dir/<attribute>/<file_name>` (one
    /// system's `SystemSpec.metadat_file`). A missing or corrupt file is logged and
    /// skipped (partial index, `complete == false`).
    pub fn load(metadat_dir: &Path, file_name: &str) -> Self {
        let mut index = Self {
            complete: true,
            ..Self::default()
        };
        for (folder, _) in ATTRIBUTE_FILES {
            let path = metadat_dir.join(folder).join(file_name);
            let parsed = std::fs::read_to_string(&path)
                .map_err(|err| err.to_string())
                .and_then(|text| parse_clrmamepro_games(&text));
            match parsed {
                Ok(games) => games.iter().for_each(|game| index.add_game(game)),
                Err(err) => {
                    tracing::warn!(file = %path.display(), %err, "metadata file not loaded");
                    index.complete = false;
                }
            }
        }
        index
    }

    /// Applies every attribute a game block carries (any of the five keys, whatever
    /// the file) to its serial (game-level, else its first ROM's) and to each ROM
    /// CRC it lists.
    fn add_game(&mut self, game: &DatGame) {
        let values: Vec<(&str, &str)> = ATTRIBUTE_FILES
            .iter()
            .filter_map(|&(folder, key)| game.attr(key).map(|value| (folder, value)))
            .collect();
        if values.is_empty() {
            return;
        }
        // Game-level serial, else the first ROM's (PSP releaseyear metadat).
        let rom_serial = || game.roms.iter().find_map(|rom| rom.serial.as_deref());
        if let Some(serial) = game.attr("serial").or_else(rom_serial) {
            let record = self.by_serial.entry(serial.to_owned()).or_default();
            values
                .iter()
                .for_each(|(f, v)| apply_attribute(record, f, v));
        }
        for crc in game.roms.iter().filter_map(|rom| rom.crc32.as_ref()) {
            let record = self.by_crc.entry(crc.clone()).or_default();
            values
                .iter()
                .for_each(|(f, v)| apply_attribute(record, f, v));
        }
    }

    /// Metadata keyed by this CRC32 (any case): a DAT entry's `metadata_crc32`.
    pub fn lookup(&self, crc32: &str) -> Option<&GameMetadataRecord> {
        self.by_crc.get(&crc32.to_ascii_uppercase())
    }

    /// Metadata of the game with this serial (`SLUS-01234`, disc systems): exact
    /// text first, then the serial rule's keys.
    pub fn lookup_serial(&self, serial: &str) -> Option<&GameMetadataRecord> {
        serial_keys(self.serial_rule, serial)
            .iter()
            .find_map(|key| self.by_serial.get(key))
    }

    /// Adds `rule`'s normalised keys for every serial (a disc system's metadat); an
    /// existing key keeps its record.
    pub fn with_serial_rule(mut self, rule: Option<DiscSignature>) -> Self {
        self.serial_rule = rule;
        let records: Vec<(String, GameMetadataRecord)> = self
            .by_serial
            .iter()
            .map(|(serial, record)| (serial.clone(), record.clone()))
            .collect();
        for (serial, record) in records {
            for key in serial_keys(rule, &serial) {
                self.by_serial.entry(key).or_insert_with(|| record.clone());
            }
        }
        self
    }

    /// Number of CRCs and serials with at least one attribute.
    pub fn len(&self) -> usize {
        self.by_crc.len() + self.by_serial.len()
    }

    /// True when nothing is loaded.
    pub fn is_empty(&self) -> bool {
        self.by_crc.is_empty() && self.by_serial.is_empty()
    }

    /// Every attribute file loaded.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Version to stamp on rows written from this index.
    pub fn source_version(&self) -> &'static str {
        if self.complete {
            METADATA_SOURCE_VERSION
        } else {
            METADATA_PARTIAL_VERSION
        }
    }
}

/// Stores one attribute value; unparsable numbers are dropped, blanks ignored.
fn apply_attribute(record: &mut GameMetadataRecord, folder: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    match folder {
        "developer" => record.developer = Some(value.to_string()),
        "publisher" => record.publisher = Some(value.to_string()),
        "genre" => record.genre = Some(value.to_string()),
        "releaseyear" => record.release_year = value.parse().ok(),
        "maxusers" => record.players = value.parse().ok().filter(|&n: &u8| n > 0),
        _ => {}
    }
}

#[cfg(test)]
#[path = "metadata_dat_index_tests.rs"]
mod tests;
