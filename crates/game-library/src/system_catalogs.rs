//! Per-system identification data (No-Intro DAT + metadat index), loaded lazily on
//! the first file of a system, so a library without NES games never parses the NES
//! DAT. Files come from the vendored `libretro-database/` folder, named by the
//! system registry (`SystemSpec.dat_file` / `metadat_file`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use storage_db::GameMetadataRecord;
use system_catalog::{SystemSpec, ALL_SYSTEMS};

use crate::dat_index::DatIndex;
use crate::metadata_dat_index::MetadataIndex;
use crate::serial_match_keys::serial_rule;

/// Reference data one system's games are identified with.
#[derive(Debug, Default)]
pub struct SystemCatalog {
    pub dat: DatIndex,
    pub metadata: MetadataIndex,
}

/// One lazily loaded [`SystemCatalog`] per registry system.
#[derive(Debug)]
pub struct SystemCatalogs {
    /// `<resources>/libretro-database`; `None` = no data (every catalog empty).
    database_dir: Option<PathBuf>,
    slots: Vec<(&'static SystemSpec, OnceLock<SystemCatalog>)>,
}

/// Returned for ids outside the registry (never loaded, always empty).
static EMPTY_CATALOG: OnceLock<SystemCatalog> = OnceLock::new();

impl SystemCatalogs {
    /// Catalogs read from `database_dir` (the folder holding the DATs and `metadat/`).
    pub fn new(database_dir: Option<PathBuf>) -> Self {
        Self {
            database_dir,
            slots: ALL_SYSTEMS
                .into_iter()
                .map(|spec| (spec, OnceLock::new()))
                .collect(),
        }
    }

    /// Pre-built catalogs (tests); systems not listed load as empty.
    pub fn from_loaded(loaded: impl IntoIterator<Item = (&'static str, SystemCatalog)>) -> Self {
        let catalogs = Self::new(None);
        for (id, catalog) in loaded {
            if let Some((_, slot)) = catalogs.slots.iter().find(|(spec, _)| spec.id == id) {
                let _ = slot.set(catalog);
            }
        }
        catalogs
    }

    /// Catalog of `platform_id`, loading it on first use (blocks concurrent callers
    /// until loaded). Missing/corrupt files give an empty index and a warning.
    pub fn get(&self, platform_id: &str) -> &SystemCatalog {
        match self.slots.iter().find(|(spec, _)| spec.id == platform_id) {
            Some((spec, slot)) => slot.get_or_init(|| self.load(spec)),
            None => EMPTY_CATALOG.get_or_init(SystemCatalog::default),
        }
    }

    /// True once `platform_id`'s catalog is loaded.
    pub fn is_loaded(&self, platform_id: &str) -> bool {
        self.slots
            .iter()
            .any(|(spec, slot)| spec.id == platform_id && slot.get().is_some())
    }

    /// Every loaded system's metadat loaded fully (rows may then carry the real
    /// source version, see `METADATA_PARTIAL_VERSION`).
    pub fn loaded_complete(&self) -> bool {
        self.slots
            .iter()
            .filter_map(|(_, slot)| slot.get())
            .all(|catalog| catalog.metadata.is_complete())
    }

    /// True when some registry DAT file is absent (file check only, nothing loaded).
    pub fn any_dat_missing(&self) -> bool {
        match &self.database_dir {
            Some(dir) => ALL_SYSTEMS
                .iter()
                .any(|spec| !dir.join(spec.dat_file).is_file()),
            None => true,
        }
    }

    /// Vendored metadata of a stored game and the version to stamp it with. Looked up
    /// by the stored `crc32`, else (re-headered NES rows store a CRC the metadat does
    /// not know) by the metadata CRC of the DAT game `dat_name`, else (disc games,
    /// metadat keyed by serial) by the stored `serial`, else the DAT game's serial.
    /// Shared by the startup self-heal and "Reset to database values".
    pub fn metadata_for_game(
        &self,
        platform_id: &str,
        dat_name: Option<&str>,
        crc32: &str,
        serial: Option<&str>,
    ) -> Option<(&GameMetadataRecord, &'static str)> {
        let catalog = self.get(platform_id);
        let by_name = dat_name.and_then(|name| catalog.dat.metadata_crc_for_name(name));
        let dat_serial = dat_name.and_then(|name| catalog.dat.serial_for_name(name));
        let record = catalog
            .metadata
            .lookup(crc32)
            .or_else(|| by_name.and_then(|crc| catalog.metadata.lookup(crc)))
            .or_else(|| serial.and_then(|s| catalog.metadata.lookup_serial(s)))
            .or_else(|| dat_serial.and_then(|s| catalog.metadata.lookup_serial(s)))?;
        Some((record, catalog.metadata.source_version()))
    }

    fn load(&self, spec: &SystemSpec) -> SystemCatalog {
        let Some(dir) = &self.database_dir else {
            return SystemCatalog::default();
        };
        let started = std::time::Instant::now();
        let dat_path = dir.join(spec.dat_file);
        let rule = serial_rule(spec);
        let dat = DatIndex::load(&dat_path)
            .unwrap_or_else(|err| {
                tracing::warn!(system = spec.id, %err, "game database missing; games will need review");
                DatIndex::empty()
            })
            .with_serial_rule(rule);
        let metadata =
            MetadataIndex::load(&metadat_dir(dir), spec.metadat_file).with_serial_rule(rule);
        tracing::info!(
            system = spec.id,
            entries = dat.len(),
            metadata = metadata.len(),
            complete = metadata.is_complete(),
            ms = started.elapsed().as_millis(),
            "system catalog loaded"
        );
        SystemCatalog { dat, metadata }
    }
}

/// Folder holding the metadat attribute folders (`developer/`, `genre/`…).
fn metadat_dir(database_dir: &Path) -> PathBuf {
    database_dir.join("metadat")
}
