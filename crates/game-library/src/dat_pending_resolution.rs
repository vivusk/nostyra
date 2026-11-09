//! DAT-gated loose files (Atari 2600 / 7800 `.bin`). Pass A (no DATs): a failure that
//! survived disc grouping (sheet tracks are hidden first) and fits a DAT-gated
//! system becomes *pending* instead of a problem. Pass B (DATs loaded): the file is
//! read and looked up; a hit is listed as that system, a miss is the old problem with
//! a hint. No rescan cache: pending files are re-read on every scan (≤ 1 MiB each).

use std::io::Read;
use std::path::Path;

use rom_identity::{dat_identified_system, dat_pending_hint};
use system_catalog::{dat_identified_candidates, SystemSpec};

use crate::disc_file_grouping::mtime_ns;
use crate::folder_scanner::{DiscoveredFile, DiscoveredFiles, ScanFailure};
use crate::system_catalogs::SystemCatalogs;

/// A loose file only a DAT can place (`DiscoveredFiles::dat_pending`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatPendingFile {
    /// UTF-8 path, as for a discovered file.
    pub path: String,
    pub size: u64,
    /// Same conversion as the walk's, so a later hit matches the stored fingerprint.
    pub mtime_ns: i64,
    /// Why classification found no system ("not a PlayStation disc").
    pub reason: String,
    /// Systems whose DAT may list it, in registry order.
    pub candidates: Vec<&'static SystemSpec>,
}

/// Moves every failure of `found` that a DAT-gated system of `systems` could take into
/// `found.dat_pending`. Call after disc grouping; a file that cannot be stat'ed stays
/// a failure.
pub(crate) fn split_dat_pending(found: &mut DiscoveredFiles, systems: &[&'static SystemSpec]) {
    let failures = std::mem::take(&mut found.failures);
    for failure in failures {
        match pending(&failure, systems) {
            Some(file) => found.dat_pending.push(file),
            None => found.failures.push(failure),
        }
    }
}

fn pending(failure: &ScanFailure, systems: &[&'static SystemSpec]) -> Option<DatPendingFile> {
    let ext = failure.path.extension()?.to_str()?;
    let path = failure.path.to_str()?;
    let meta = std::fs::metadata(&failure.path)
        .ok()
        .filter(|m| m.is_file())?;
    let candidates = dat_identified_candidates(systems, ext, meta.len());
    (!candidates.is_empty()).then(|| DatPendingFile {
        path: path.to_string(),
        size: meta.len(),
        mtime_ns: mtime_ns(&meta),
        reason: failure.reason.clone(),
        candidates,
    })
}

/// Pass B: looks every pending file of `discovered` up in its candidates' DATs. A hit
/// joins `discovered.files` (then hashed or kept unchanged like any file), a miss or a
/// read error joins `discovered.failures`.
pub(crate) fn resolve_dat_pending(discovered: &mut DiscoveredFiles, catalogs: &SystemCatalogs) {
    for file in std::mem::take(&mut discovered.dat_pending) {
        let failure = |reason: String| ScanFailure {
            path: file.path.clone().into(),
            reason,
        };
        let limit = file
            .candidates
            .iter()
            .map(|s| s.max_bytes)
            .max()
            .unwrap_or(0);
        let bytes = match read_capped(Path::new(&file.path), limit) {
            Ok(bytes) => bytes,
            Err(err) => {
                discovered.failures.push(failure(err.to_string()));
                continue;
            }
        };
        let dat_for = |spec: &SystemSpec| Some(&catalogs.get(spec.id).dat);
        match dat_identified_system(&bytes, &file.candidates, dat_for) {
            Some(system) => discovered.files.push(DiscoveredFile {
                path: file.path.clone(),
                size: file.size,
                mtime_ns: file.mtime_ns,
                system,
            }),
            None => discovered
                .failures
                .push(failure(dat_pending_hint(&file.reason, &file.candidates))),
        }
    }
}

/// The whole file, at most `limit` + 1 bytes (a file that grew is a miss, not a hog).
fn read_capped(path: &Path, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}
