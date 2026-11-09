//! One game's sync result for the UI ([`SyncGameResult`]) from its units' results:
//! conflicts win (with the dialog data), then offline, then other problems.

use std::path::Path;

use cloud_sync::{
    CloudError, Conflict, RemoteManifest, SyncStatus, SyncUnit, SyncUnitError, MANIFEST_FILE,
};
use serde::Serialize;

use crate::cloud_game_sync::UnitResult;
use crate::cloud_provider_factory::CloudAccount;

/// `cloud_sync_game` / `cloud_resolve_conflict` result for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SyncGameResult {
    Synced,
    /// Cloud unreachable / too slow / cancelled: play with this PC's saves.
    Offline,
    /// Both sides changed: the user picks (nothing was written).
    Conflict {
        files: Vec<String>,
        /// Newest local mtime of the conflicted files (unix seconds).
        local_time: Option<i64>,
        remote_time: Option<i64>,
        remote_device: Option<String>,
    },
    Skipped {
        reason: String,
    },
}

/// One game's result from its units' results (`units[i]` ↔ `results[i]`).
pub fn game_result(
    account: &CloudAccount,
    units: &[SyncUnit],
    results: &[UnitResult],
) -> SyncGameResult {
    let mut conflicts = Vec::new();
    let mut offline = false;
    let mut problem = None;
    let mut in_use = 0;
    for (unit, result) in units.iter().zip(results) {
        match result {
            UnitResult::Done(outcome) if outcome.status == SyncStatus::NeedsChoice => {
                conflicts.push((unit, outcome.conflicts.clone()));
            }
            UnitResult::Done(outcome) => match &outcome.status {
                SyncStatus::Busy => offline = true,
                SyncStatus::RemoteInconsistent { .. } => {
                    problem = Some("cloud copy damaged; kept this PC's saves".to_string());
                }
                _ => {}
            },
            UnitResult::InUse => in_use += 1,
            UnitResult::Stopped => offline = true,
            UnitResult::Failed(err) => match error_kind(err) {
                ErrorKind::Offline => offline = true,
                ErrorKind::SignIn => problem = Some("sign in again".to_string()),
                ErrorKind::Other => problem = Some(err.to_string()),
            },
        }
    }
    if !conflicts.is_empty() {
        return conflict_result(account, &conflicts);
    }
    if offline {
        return SyncGameResult::Offline;
    }
    if let Some(reason) = problem {
        return SyncGameResult::Skipped { reason };
    }
    if in_use > 0 && in_use == units.len() {
        return SyncGameResult::Skipped {
            reason: "in use".to_string(),
        };
    }
    SyncGameResult::Synced
}

/// How a failed unit is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Network / throttled / cancelled: retried later, play goes on.
    Offline,
    /// The provider refused the sign-in.
    SignIn,
    Other,
}

pub fn error_kind(err: &SyncUnitError) -> ErrorKind {
    match err {
        SyncUnitError::Cancelled => ErrorKind::Offline,
        SyncUnitError::Cloud(err) if err.is_transient() => ErrorKind::Offline,
        SyncUnitError::Cloud(CloudError::AuthExpired) => ErrorKind::SignIn,
        _ => ErrorKind::Other,
    }
}

/// The conflict dialog data: conflicted paths, newest local mtime, the cloud side's
/// writer + time (one manifest read; informative only).
fn conflict_result(
    account: &CloudAccount,
    conflicts: &[(&SyncUnit, Vec<Conflict>)],
) -> SyncGameResult {
    let mut files = Vec::new();
    let mut local_time = None;
    let mut remote: Option<RemoteManifest> = None;
    for (unit, list) in conflicts {
        for conflict in list {
            files.push(conflict.path.clone());
            let path = unit.local_dir().join(&conflict.path);
            local_time = local_time.max(modified_secs(&path));
        }
        let manifest_path = format!("{}{MANIFEST_FILE}", unit.remote_prefix());
        let read = account.provider.read(&manifest_path).ok();
        let manifest = read.and_then(|(bytes, _)| RemoteManifest::parse(&bytes).ok());
        if manifest.as_ref().map(|m| m.updated_at) > remote.as_ref().map(|m| m.updated_at) {
            remote = manifest;
        }
    }
    SyncGameResult::Conflict {
        files,
        local_time,
        remote_time: remote
            .as_ref()
            .and_then(|m| i64::try_from(m.updated_at).ok()),
        remote_device: remote.map(|m| m.device_name),
    }
}

fn modified_secs(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    i64::try_from(secs).ok()
}
