//! Walks a library folder and lists ROM files with their system and fingerprints.
//! Each file is classified by content (extension → candidate systems → size →
//! content probe on the first bytes); folder names are never read, so users can
//! organise ROMs any way (`ROM/Favorites/x.nes`, an `.sfc` inside `ROM/NES/`).

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use disc_image::DiscImageProbe;
use system_catalog::{classify_among, ClassifyError, Media, SystemSpec};

use crate::disc_file_grouping::group_disc_files;
use walkdir::WalkDir;

/// A ROM file of a known system within its size limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFile {
    /// UTF-8 path as stored in storage-db (`games.rom_path`).
    pub path: String,
    pub size: u64,
    /// Modification time in ns since the unix epoch (0 if the OS cannot tell).
    pub mtime_ns: i64,
    /// System decided from the file content.
    pub system: &'static SystemSpec,
}

/// A file the scan could not use, with a readable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanFailure {
    pub path: PathBuf,
    pub reason: String,
}

/// Result of walking one folder.
#[derive(Debug, Default)]
pub struct DiscoveredFiles {
    pub files: Vec<DiscoveredFile>,
    /// Matching extension but outside every candidate system's size limits.
    pub skipped: usize,
    /// Unreadable entries, non-UTF-8 paths and failed content probes ("missing
    /// iNES header"); the walk continues past them.
    pub failures: Vec<ScanFailure>,
}

/// Lists files under `folder` that belong to one of `systems`. Files whose extension
/// no system in `systems` uses are ignored silently. Symlinks are not followed (no
/// loops); errors are collected per entry, never abort the walk. Disc games made of
/// several files (`.cue` + tracks, `.m3u` + discs) come out as one entry.
pub fn discover_rom_files(folder: &Path, systems: &[&'static SystemSpec]) -> DiscoveredFiles {
    let mut found = DiscoveredFiles::default();
    for entry in WalkDir::new(folder).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                found.failures.push(ScanFailure {
                    path: err.path().unwrap_or(folder).to_path_buf(),
                    reason: err.to_string(),
                });
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(ext) = entry.path().extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if !systems.iter().any(|spec| spec.has_extension(ext)) {
            continue;
        }
        let failure = |reason: String| ScanFailure {
            path: entry.path().to_path_buf(),
            reason,
        };
        let Some(path) = entry.path().to_str() else {
            found
                .failures
                .push(failure("path is not valid UTF-8".into()));
            continue;
        };
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(err) => {
                found.failures.push(failure(err.to_string()));
                continue;
            }
        };
        // The classifier opens the file only when a size-fitting candidate checks
        // content (one small head read per `.nes`, a few sectors per disc image).
        let system =
            match classify_among(systems, ext, metadata.len(), entry.path(), &DiscImageProbe) {
                Ok(system) => system,
                // `NotRom`: content shows no ROM at all (a `README.md`), not a problem.
                Err(
                    ClassifyError::OutOfSize
                    | ClassifyError::NoCandidate
                    | ClassifyError::NotRom(_),
                ) => {
                    found.skipped += 1;
                    continue;
                }
                Err(err) => {
                    found.failures.push(failure(err.to_string()));
                    continue;
                }
            };
        let mtime_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |since| {
                i64::try_from(since.as_nanos()).unwrap_or(i64::MAX)
            });
        found.files.push(DiscoveredFile {
            path: path.to_string(),
            size: metadata.len(),
            mtime_ns,
            system,
        });
    }
    if systems.iter().any(|spec| spec.media == Media::Disc) {
        group_disc_files(&mut found, systems);
    }
    found
}
