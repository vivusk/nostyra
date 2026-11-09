//! Keeps the runner and the cloud sync apart, per **sync unit** (not per game): a
//! running session holds its game unit plus the shared PS2 memory cards when the
//! game is a PS2 game. Play never waits on an upload: a job in `Transferring` on one
//! of its units is cancelled (staging / network only), a job in `Applying` (renames,
//! ms-scale) is waited for. Every other writer of a game folder / the memcards
//! refuses (`busy`) while a cloud job holds one of the game's units.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cloud_sync::{BaseManifestStore, SyncPhase, SyncUnit};

use crate::app_state::ActiveSession;
use crate::cloud_sync_state::CloudSyncState;
use crate::command_error::{CommandError, CommandResult};
use crate::commands::game_data_target::{ensure_idle, GameDataTarget};

/// Poll interval while Play waits for an `Applying` pass.
const APPLY_POLL: Duration = Duration::from_millis(5);

/// Whether `system` keeps its saves in the shared PS2 memory-card folder.
pub fn uses_shared_ps2_memcards(system: &str) -> bool {
    system_catalog::PCEE2.systems.contains(&system)
}

/// The sync units of a game: its save folder (when it has a content id and a ROM to
/// key the folder on) + the shared PS2 memory cards for PS2 games. `Err(reason)`
/// when the game has no unit of its own.
pub fn game_sync_units(
    data_root: &Path,
    target: &GameDataTarget,
) -> Result<Vec<SyncUnit>, &'static str> {
    let dir = target.saves_dir(data_root).ok_or("no save folder")?;
    let unit = SyncUnit::game(&target.system, &target.content_sha1, dir).ok_or("no content id")?;
    let mut units = vec![unit];
    if uses_shared_ps2_memcards(&target.system) {
        units.push(SyncUnit::ps2_memcards(data_root));
    }
    Ok(units)
}

/// Unit keys a session of this game holds (also without a save folder: the keys
/// depend on system + content sha1 only).
pub fn session_unit_keys(target: &GameDataTarget) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Some(unit) = SyncUnit::game(&target.system, &target.content_sha1, PathBuf::new()) {
        keys.insert(unit.key());
    }
    if uses_shared_ps2_memcards(&target.system) {
        keys.insert(SyncUnit::ps2_memcards(Path::new("")).key());
    }
    keys
}

/// What Play found on its units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayGuard {
    /// No job on the game's units.
    Free,
    /// A transfer was cancelled (nothing local touched; re-synced later).
    CancelledTransfer,
    /// An apply pass was running; Play waited for it to end.
    WaitedForApply,
}

/// Called by `play_game` **with the session lock held** (lock order session → cloud
/// job): cancels / waits for a job on `keys`. Once cancelled, the job can never start
/// another apply pass (`SyncProgress::cancel_and_phase`), so the runner starts on
/// folders nothing else writes.
pub fn clear_units_for_play(cloud: &CloudSyncState, keys: &BTreeSet<String>) -> PlayGuard {
    let Some(progress) = cloud.job_progress_for(keys) else {
        return PlayGuard::Free;
    };
    if progress.cancel_and_phase() != SyncPhase::Applying {
        tracing::info!(target: "cloud_sync", "play: cancelled a cloud transfer");
        return PlayGuard::CancelledTransfer;
    }
    while progress.phase() == SyncPhase::Applying {
        std::thread::sleep(APPLY_POLL);
    }
    tracing::info!(target: "cloud_sync", "play: waited for a cloud apply pass");
    PlayGuard::WaitedForApply
}

/// `busy` while the game runs (`ensure_idle`) or a cloud job holds one of its units.
/// Callers hold the session lock across their file work, so neither a Play nor a
/// new cloud unit can start in between.
pub fn ensure_unit_idle(
    session: Option<&ActiveSession>,
    cloud: &CloudSyncState,
    game_id: i64,
    target: &GameDataTarget,
) -> CommandResult<()> {
    ensure_idle(session.map(|s| s.game_id), game_id)?;
    if cloud.job_progress_for(&session_unit_keys(target)).is_some() {
        return Err(CommandError::new(
            "busy",
            "Cloud saves are syncing this game right now. Try again in a moment.",
        ));
    }
    Ok(())
}

/// Missing-game removal (r2 F1): deletes the game unit's base manifests of every
/// account (`<data>/cloud/base/*/<sys>/<sha1>.json`). The cloud copy stays; a re-add
/// then syncs with no base (= pull), never as deletions.
pub fn remove_unit_bases(data_root: &Path, system: &str, content_sha1: &str) -> io::Result<()> {
    let Some(unit) = SyncUnit::game(system, content_sha1, PathBuf::new()) else {
        return Ok(());
    };
    let accounts = match std::fs::read_dir(data_root.join("cloud").join("base")) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    for entry in accounts {
        let entry = entry?;
        let Some(account) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let base = BaseManifestStore::new(data_root, &account).path_for(&unit);
        match std::fs::remove_file(&base) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
    }
    Ok(())
}
