//! Scan pass A for one library folder: walk it once (every registry system,
//! classified by content), then flag its vanished files missing. Running pass A over
//! **all** folders before any upsert means a file moved from folder B to folder A
//! finds its old row already missing and relinks, whatever the folder order.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use storage_db::{
    file_fingerprints, mark_missing_paths, Connection, FileFingerprint, LibraryFolder,
};
use system_catalog::SystemSpec;

use crate::folder_scanner::{discover_rom_files, DiscoveredFiles};
use crate::library_error::LibraryError;
use crate::library_scan_job::lock;

/// What pass A found in one folder; pass B hashes and upserts from it (no second walk).
#[derive(Debug)]
pub(crate) struct FolderDiscovery {
    pub folder: LibraryFolder,
    pub discovered: DiscoveredFiles,
    /// Fingerprints of the folder's present rows before this scan (rescan cache).
    pub known: HashMap<String, FileFingerprint>,
    /// Rows flagged missing by this pass.
    pub missing: usize,
}

/// Walks `folder` and marks its vanished paths missing. A missing folder root is an
/// error and leaves the folder's rows untouched.
pub(crate) fn discover_folder(
    db: &Mutex<Connection>,
    folder: &LibraryFolder,
    systems: &[&'static SystemSpec],
) -> Result<FolderDiscovery, LibraryError> {
    let root = Path::new(&folder.path);
    if !root.is_dir() {
        return Err(LibraryError::FolderNotFound(root.to_path_buf()));
    }
    let discovered = discover_rom_files(root, systems);
    let known = file_fingerprints(&lock(db), folder.id)?;
    let present: HashSet<&str> = discovered.files.iter().map(|f| f.path.as_str()).collect();
    // Entries the walk could not read are unknown, not vanished: keep their games.
    let unreadable: Vec<PathBuf> = discovered.failures.iter().map(|f| f.path.clone()).collect();
    let vanished: Vec<String> = known
        .keys()
        .filter(|path| !present.contains(path.as_str()))
        .filter(|path| {
            !unreadable
                .iter()
                .any(|bad| Path::new(path).starts_with(bad))
        })
        .cloned()
        .collect();
    let missing = mark_missing_paths(&mut lock(db), &vanished)?;
    Ok(FolderDiscovery {
        folder: folder.clone(),
        discovered,
        known,
        missing,
    })
}
