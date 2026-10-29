//! Temp library folders and databases for game-library integration tests.
#![allow(dead_code)] // each test crate uses a different subset
#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

pub mod ps1_test_tree;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use game_library::{
    scan_library, DatIndex, MetadataIndex, ScanSummary, SystemCatalog, SystemCatalogs,
};
use storage_db::{add_library_folder, open_database, Connection, LibraryFolder};

/// A unique temp dir holding `roms/` and `nostyra.db`; removed on drop.
pub struct TempLibrary {
    pub root: PathBuf,
    pub db: Mutex<Connection>,
    pub folder: LibraryFolder,
    /// Must stay the last field: fields drop in declaration order, and on Windows
    /// the dir cannot be removed while the SQLite connection still holds `nostyra.db` open.
    _temp: tempfile::TempDir,
}

impl TempLibrary {
    pub fn new(name: &str) -> Self {
        let temp = tempfile::Builder::new()
            .prefix(&format!("nostyra-gl-{name}-"))
            .tempdir()
            .unwrap();
        let root = temp.path().to_path_buf();
        let roms = root.join("roms");
        std::fs::create_dir_all(&roms).unwrap();
        let conn = open_database(&root.join("nostyra.db")).unwrap();
        let folder = add_library_folder(&conn, roms.to_str().unwrap(), 1).unwrap();
        Self {
            root,
            db: Mutex::new(conn),
            folder,
            _temp: temp,
        }
    }

    pub fn roms(&self) -> PathBuf {
        self.root.join("roms")
    }

    pub fn write(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = self.roms().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }

    pub fn scan(&self, dat: &DatIndex) -> ScanSummary {
        self.scan_with(dat, &MetadataIndex::empty())
    }

    pub fn scan_with(&self, dat: &DatIndex, metadata: &MetadataIndex) -> ScanSummary {
        scan_library(
            &self.db,
            std::slice::from_ref(&self.folder),
            &snes_catalogs(dat, metadata),
            10,
            &|_| {},
            &AtomicBool::new(false),
        )
        .unwrap()
    }
}

/// Deterministic pseudo-ROM bytes, distinct per `seed`.
pub fn fake_rom(len: usize, seed: u32) -> Vec<u8> {
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect()
}

/// The repo's self-written test ROM (32 KiB, not in any DAT).
pub fn test_rom_bytes() -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms/nostyra-test.sfc"),
    )
    .unwrap()
}

/// Catalogs holding `dat` + `metadata` as the SNES catalog (other systems empty).
pub fn snes_catalogs(dat: &DatIndex, metadata: &MetadataIndex) -> SystemCatalogs {
    SystemCatalogs::from_loaded([(
        "snes",
        SystemCatalog {
            dat: dat.clone(),
            metadata: metadata.clone(),
        },
    )])
}
