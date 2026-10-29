//! Hash lookup over a parsed DAT, plus card title/region helpers.

use std::collections::HashMap;
use std::path::Path;

use system_catalog::DiscSignature;

use crate::clrmamepro_dat_parser::{parse_clrmamepro_dat, DatEntry};
use crate::library_error::LibraryError;
use crate::rom_file_hasher::RomHashes;

/// DAT entries indexed by SHA1, with a CRC32+size fallback for entries without SHA1,
/// a game-name index (metadata CRC of a stored `dat_name`) and, for disc DATs, a
/// serial index (several entries may share one serial: revisions, regional twins).
#[derive(Debug, Default, Clone)]
pub struct DatIndex {
    pub(crate) entries: Vec<DatEntry>,
    by_sha1: HashMap<String, usize>,
    by_crc_size: HashMap<(String, u64), usize>,
    by_name: HashMap<String, usize>,
    pub(crate) by_serial: HashMap<String, Vec<usize>>,
    /// Disc systems: serials also match under the signature's normalised keys.
    pub(crate) serial_rule: Option<DiscSignature>,
}

impl DatIndex {
    /// No DAT available: every game becomes "needs review"; scanning still works.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Loads and indexes a DAT file. Callers fall back to [`DatIndex::empty`] on error.
    pub fn load(path: &Path) -> Result<Self, LibraryError> {
        let text = std::fs::read_to_string(path).map_err(|source| LibraryError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let entries = parse_clrmamepro_dat(&text).map_err(|message| LibraryError::DatParse {
            path: path.to_path_buf(),
            message,
        })?;
        Ok(Self::from_entries(entries))
    }

    /// Indexes entries; on duplicate hashes the first entry wins (stable results).
    pub fn from_entries(entries: Vec<DatEntry>) -> Self {
        let mut by_sha1 = HashMap::with_capacity(entries.len());
        let mut by_crc_size = HashMap::with_capacity(entries.len());
        let mut by_name = HashMap::with_capacity(entries.len());
        for (i, entry) in entries.iter().enumerate() {
            by_name.entry(entry.name.clone()).or_insert(i);
            if let Some(sha1) = &entry.sha1 {
                by_sha1.entry(sha1.clone()).or_insert(i);
            }
            by_crc_size
                .entry((entry.crc32.clone(), entry.size))
                .or_insert(i);
        }
        let mut index = Self {
            entries,
            by_sha1,
            by_crc_size,
            by_name,
            by_serial: HashMap::new(),
            serial_rule: None,
        };
        index.index_serials();
        index
    }

    /// Entry with this exact SHA1 (first in DAT order when several share it).
    pub fn lookup_sha1(&self, sha1: &str) -> Option<&DatEntry> {
        self.by_sha1.get(sha1).map(|&i| &self.entries[i])
    }

    /// Serial of the DAT game called `name` (a stored `games.dat_name`).
    pub fn serial_for_name(&self, name: &str) -> Option<&str> {
        self.by_name
            .get(name)
            .and_then(|&i| self.entries[i].serial.as_deref())
    }

    /// Metadata CRC of the DAT game called `name` (a stored `games.dat_name`), so
    /// re-headered NES rows find their metadat record without the file.
    pub fn metadata_crc_for_name(&self, name: &str) -> Option<&str> {
        self.by_name
            .get(name)
            .map(|&i| self.entries[i].metadata_crc32.as_str())
    }

    /// Match order: SHA1, then CRC32 + size.
    pub fn lookup(&self, hashes: &RomHashes) -> Option<&DatEntry> {
        self.by_sha1
            .get(&hashes.sha1)
            .or_else(|| self.by_crc_size.get(&(hashes.crc32.clone(), hashes.size)))
            .map(|&i| &self.entries[i])
    }

    /// Number of ROM entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no DAT is loaded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Card title: name without trailing `(…)` / `[…]` tags ("Super Mario World (USA)" →
/// "Super Mario World"). Also used for unmatched file stems ("Game (U) [!]").
pub fn display_title(full_name: &str) -> String {
    let cut = full_name
        .find(" (")
        .into_iter()
        .chain(full_name.find(" ["))
        .min()
        .unwrap_or(full_name.len());
    let title = full_name[..cut].trim();
    if title.is_empty() {
        full_name.trim().to_string()
    } else {
        title.to_string()
    }
}

/// Region from the DAT `region` field, else the first tag naming a known region.
pub fn display_region(entry: &DatEntry) -> Option<String> {
    const REGIONS: &[&str] = &[
        "USA",
        "Japan",
        "Europe",
        "World",
        "Germany",
        "France",
        "Spain",
        "Italy",
        "Korea",
        "Brazil",
        "Australia",
        "Asia",
        "China",
        "Hong Kong",
        "Taiwan",
        "Canada",
        "Sweden",
        "Netherlands",
    ];
    if let Some(region) = &entry.region {
        return Some(region.clone());
    }
    entry
        .name
        .split('(')
        .skip(1)
        .filter_map(|tag| tag.split(')').next())
        .find(|tag| tag.split(", ").all(|part| REGIONS.contains(&part)))
        .map(str::to_string)
}
