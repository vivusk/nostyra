//! Offline game metadata from the vendored libretro-database `metadat/` attribute DATs
//! (one folder per attribute), merged into two lookups: by ROM CRC32 (No-Intro
//! systems: each file carries its own attribute) and by game `serial` (PlayStation:
//! one serial-keyed file carries every attribute, ROMs have no CRC). Shared by the
//! desktop scanner (files from disk, [`MetadataIndex::load`]) and the web edition
//! (texts fetched by the browser, [`MetadataIndex::from_texts`]).

use std::collections::HashMap;
use std::path::Path;

use system_catalog::DiscSignature;

use crate::clrmamepro_dat_parser::{parse_clrmamepro_games, DatGame};
use crate::metadata_record::{apply_attribute, MetadataRecord};
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
    by_crc: HashMap<String, MetadataRecord>,
    by_serial: HashMap<String, MetadataRecord>,
    /// Disc systems: serials also match under the signature's normalised keys.
    serial_rule: Option<DiscSignature>,
    /// False when any attribute file was missing or corrupt (scans and self-heal then
    /// stamp the "+partial" version, so the next full load re-heals).
    complete: bool,
}

/// The attribute folders under `metadat/` (`developer`, `genre`…), in load order.
pub fn metadata_attribute_folders() -> [&'static str; 5] {
    ATTRIBUTE_FILES.map(|(folder, _)| folder)
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
        Self::from_texts(ATTRIBUTE_FILES.map(|(folder, _)| {
            let path = metadat_dir.join(folder).join(file_name);
            let text =
                std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()));
            (folder, text)
        }))
    }

    /// Builds the index from the attribute files' texts, `(folder, text or read
    /// error)` per [`metadata_attribute_folders`] entry. A missing or corrupt file is
    /// logged and skipped (partial index, `complete == false`).
    pub fn from_texts<'a>(
        files: impl IntoIterator<Item = (&'a str, Result<String, String>)>,
    ) -> Self {
        let mut index = Self {
            complete: true,
            ..Self::default()
        };
        for (folder, text) in files {
            match text.and_then(|text| parse_clrmamepro_games(&text)) {
                Ok(games) => games.iter().for_each(|game| index.add_game(game)),
                Err(err) => {
                    tracing::warn!(folder, %err, "metadata file not loaded");
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
    pub fn lookup(&self, crc32: &str) -> Option<&MetadataRecord> {
        self.by_crc.get(&crc32.to_ascii_uppercase())
    }

    /// Metadata of the game with this serial (`SLUS-01234`, disc systems): exact
    /// text first, then the serial rule's keys.
    pub fn lookup_serial(&self, serial: &str) -> Option<&MetadataRecord> {
        serial_keys(self.serial_rule, serial)
            .iter()
            .find_map(|key| self.by_serial.get(key))
    }

    /// Adds `rule`'s normalised keys for every serial (a disc system's metadat); an
    /// existing key keeps its record.
    pub fn with_serial_rule(mut self, rule: Option<DiscSignature>) -> Self {
        self.serial_rule = rule;
        let records: Vec<(String, MetadataRecord)> = self
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

#[cfg(test)]
#[path = "metadata_dat_index_tests.rs"]
mod tests;
