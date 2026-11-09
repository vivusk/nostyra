//! Walks a library folder and lists ROM files with their system and fingerprints.
//! Each file is classified by content (extension → candidate systems → size →
//! content probe on the first bytes); folder names are never read, so users can
//! organise ROMs any way (`ROM/Favorites/x.nes`, an `.sfc` inside `ROM/NES/`).

use std::path::{Path, PathBuf};

use disc_image::DiscImageProbe;
use system_catalog::{classify_among, ClassifyError, Media, SystemSpec};

use crate::dat_pending_resolution::{split_dat_pending, DatPendingFile};
use crate::disc_file_grouping::{group_disc_files, mtime_ns};
use crate::library_scan_options::ScanOptions;
use crate::rom_archive::is_rom_archive;
use rom_identity::{scan_zip, ZipScan};
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
    /// Unclassified loose files that may match a DAT (Atari `.bin`); resolved in pass
    /// B, not listed in `failures`.
    pub dat_pending: Vec<DatPendingFile>,
}

/// Lists files under `folder` that belong to one of `systems`. Files whose extension
/// no system in `systems` uses are ignored silently. Symlinks are not followed (no
/// loops); errors are collected per entry, never abort the walk. Disc games made of
/// several files (`.cue` + tracks, `.m3u` + discs) come out as one entry.
pub fn discover_rom_files(folder: &Path, systems: &[&'static SystemSpec]) -> DiscoveredFiles {
    discover_rom_files_with_options(folder, systems, ScanOptions::default())
}

/// Like [`discover_rom_files`], optionally inspecting single-ROM ZIP archives.
pub fn discover_rom_files_with_options(
    folder: &Path,
    systems: &[&'static SystemSpec],
    options: ScanOptions,
) -> DiscoveredFiles {
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
        let archived = options.scan_zip_archives && is_rom_archive(entry.path());
        if !archived && !systems.iter().any(|spec| spec.has_extension(ext)) {
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
        let classified = if archived {
            // Another system or not a single-ROM ZIP (arcade set, several ROMs):
            // skipped quietly like any non-ROM file.
            match scan_zip(entry.path(), systems) {
                ZipScan::Rom(system) => Ok(system),
                ZipScan::Skip => Err(ClassifyError::NoCandidate),
                ZipScan::Problem(reason) => Err(ClassifyError::Io(reason)),
            }
        } else {
            classify_among(systems, ext, metadata.len(), entry.path(), &DiscImageProbe)
        };
        let system = match classified {
            Ok(system) => system,
            // `NotRom`: content shows no ROM at all (a `README.md`), not a problem.
            Err(
                ClassifyError::OutOfSize | ClassifyError::NoCandidate | ClassifyError::NotRom(_),
            ) => {
                found.skipped += 1;
                continue;
            }
            Err(err) => {
                found.failures.push(failure(err.to_string()));
                continue;
            }
        };
        found.files.push(DiscoveredFile {
            path: path.to_string(),
            size: metadata.len(),
            mtime_ns: mtime_ns(&metadata),
            system,
        });
    }
    if systems.iter().any(|spec| spec.media == Media::Disc) {
        group_disc_files(&mut found, systems);
    }
    // After grouping (sheet tracks are hidden, never pending), on every scan.
    split_dat_pending(&mut found, systems);
    found
}
