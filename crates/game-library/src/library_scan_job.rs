//! Scans every library folder in two passes: A = walk + mark vanished files missing
//! (all folders, see `library_scan_discovery`), B = per folder skip unchanged → hash
//! (parallel) → DAT match → batched upsert (moved/renamed files relink by sha1).
//! Holds the database lock only per 50-file batch so the UI can list games while a
//! scan runs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use rayon::prelude::*;
use storage_db::{upsert_scanned_batch, Connection, FileFingerprint, LibraryFolder, RelinkedGame};
use system_catalog::ALL_SYSTEMS;

use crate::dat_rematch::rematch_unidentified;
use crate::folder_scanner::{DiscoveredFile, ScanFailure};
use crate::library_error::LibraryError;
use crate::library_scan_discovery::{discover_folder, FolderDiscovery};
use crate::scanned_game_builder::build_scanned_game;
use crate::system_catalogs::SystemCatalogs;

/// Files per database transaction.
const BATCH_SIZE: usize = 50;
/// Hashing is I/O bound; more threads just thrash the disk.
const MAX_HASH_THREADS: usize = 4;

/// Progress for the UI scan banner. `files_done` counts unchanged files too; pass A
/// ("looking for games") reports each folder with `files_total == 0`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanProgress {
    pub folder_path: String,
    pub files_total: usize,
    pub files_done: usize,
    pub current_path: Option<String>,
}

/// What a scan did. `matched`/`unmatched` count the files hashed in this run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanSummary {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub relinked: usize,
    /// Every relink of this scan (old → new path), for the launcher's save-folder move.
    pub relinked_games: Vec<RelinkedGame>,
    pub missing: usize,
    pub skipped: usize,
    pub matched: usize,
    pub unmatched: usize,
    /// Unchanged games the DAT identified this run (no re-hash needed).
    pub rematched: usize,
    pub failures: Vec<ScanFailure>,
    /// `(folder path, error)` of folders that could not be scanned (missing root,
    /// database error); their rows are left as they were.
    pub folder_errors: Vec<(String, String)>,
    pub cancelled: bool,
}

/// Scans `folders` for games of every registry system and writes the results
/// (metadata of matched games included). `now` (unix seconds) stamps new games.
/// Only a worker-pool failure aborts; folder problems go to `folder_errors`.
pub fn scan_library(
    db: &Mutex<Connection>,
    folders: &[LibraryFolder],
    catalogs: &SystemCatalogs,
    now: i64,
    progress: &(dyn Fn(&ScanProgress) + Sync),
    cancel: &AtomicBool,
) -> Result<ScanSummary, LibraryError> {
    let mut summary = ScanSummary::default();
    let mut discoveries = Vec::with_capacity(folders.len());
    for folder in folders {
        if cancel.load(Ordering::Relaxed) {
            // Pass A partial: only `missing` flags were written; the next scan heals.
            summary.cancelled = true;
            return Ok(summary);
        }
        progress(&ScanProgress {
            folder_path: folder.path.clone(),
            ..ScanProgress::default()
        });
        match discover_folder(db, folder, &ALL_SYSTEMS) {
            Ok(found) => discoveries.push(found),
            Err(err) => summary
                .folder_errors
                .push((folder.path.clone(), err.to_string())),
        }
    }

    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(MAX_HASH_THREADS));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|err| LibraryError::ThreadPool(err.to_string()))?;
    for found in discoveries {
        let path = found.folder.path.clone();
        // Accumulates in place, so work committed before a storage error is kept.
        if let Err(err) = scan_discovered_folder(
            db,
            found,
            catalogs,
            now,
            progress,
            cancel,
            &pool,
            &mut summary,
        ) {
            summary.folder_errors.push((path, err.to_string()));
        }
        if summary.cancelled {
            break;
        }
    }
    Ok(summary)
}

/// Pass B for one folder, adding its results to `summary` as they happen.
#[allow(clippy::too_many_arguments)]
fn scan_discovered_folder(
    db: &Mutex<Connection>,
    found: FolderDiscovery,
    catalogs: &SystemCatalogs,
    now: i64,
    progress: &(dyn Fn(&ScanProgress) + Sync),
    cancel: &AtomicBool,
    pool: &rayon::ThreadPool,
    summary: &mut ScanSummary,
) -> Result<(), LibraryError> {
    let FolderDiscovery {
        folder,
        discovered,
        known,
        missing,
    } = found;
    summary.missing += missing;
    summary.skipped += discovered.skipped;
    summary.failures.extend(discovered.failures);
    let (unchanged, to_hash): (Vec<&DiscoveredFile>, Vec<&DiscoveredFile>) = discovered
        .files
        .iter()
        .partition(|file| known.get(&file.path) == Some(&fingerprint(file)));
    summary.unchanged += unchanged.len();
    summary.rematched += rematch_unidentified(db, folder.id, &unchanged, catalogs, now)?;
    let mut state = ScanProgress {
        folder_path: folder.path.clone(),
        files_total: discovered.files.len(),
        files_done: unchanged.len(),
        current_path: None,
    };
    progress(&state);

    for batch in to_hash.chunks(BATCH_SIZE) {
        if cancel.load(Ordering::Relaxed) {
            summary.cancelled = true;
            break;
        }
        let results: Vec<_> = pool.install(|| {
            batch
                .par_iter()
                .map(|file| build_scanned_game(file, folder.id, catalogs.get(file.system.id)))
                .collect()
        });
        let mut games = Vec::with_capacity(results.len());
        for result in results {
            match result {
                Ok(game) => games.push(game),
                Err(failure) => summary.failures.push(failure),
            }
        }
        summary.matched += games.iter().filter(|g| g.dat_name.is_some()).count();
        summary.unmatched += games.iter().filter(|g| g.dat_name.is_none()).count();
        let outcome = upsert_scanned_batch(&mut lock(db), &games, now)?;
        summary.added += outcome.inserted;
        summary.updated += outcome.updated;
        summary.relinked += outcome.relinked.len();
        summary.relinked_games.extend(outcome.relinked);

        state.files_done += batch.len();
        state.current_path = batch.last().map(|file| file.path.clone());
        progress(&state);
    }
    Ok(())
}

fn fingerprint(file: &DiscoveredFile) -> FileFingerprint {
    FileFingerprint {
        size: i64::try_from(file.size).unwrap_or(i64::MAX),
        mtime_ns: file.mtime_ns,
    }
}

/// A panic while holding the lock cannot leave SQLite half-written (transactions roll
/// back), so a poisoned mutex is safe to keep using.
pub(crate) fn lock(db: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    db.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
