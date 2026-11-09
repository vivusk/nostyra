//! Inputs and results of one unit sync ([`crate::sync_unit`]).

use std::path::Path;

use crate::base_manifest_store::BaseManifestStore;
use crate::cloud_error::CloudError;
use crate::cloud_storage_provider::CloudStorageProvider;
use crate::cloud_sync_error::CloudSyncError;
use crate::conflict_choice::Choice;
use crate::device_identity::DeviceIdentity;
use crate::sync_progress::SyncProgress;
use crate::sync_scope::SyncScope;
use crate::three_way_sync_planner::Conflict;

/// Which half of a sync runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    /// Pull, then push + commit + index + GC.
    Both,
    /// Before play: pull / local deletes / adopt only; nothing is uploaded, pending
    /// local changes and "Keep this PC" choices are pushed by the next `Both` run.
    PullOnly,
}

/// How to run one unit sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncOptions {
    pub direction: SyncDirection,
    /// The user's answer to this unit's conflicts; `None` = return them unsolved.
    pub choice: Option<Choice>,
}

impl SyncOptions {
    /// Full sync, no choice.
    pub fn both() -> Self {
        Self {
            direction: SyncDirection::Both,
            choice: None,
        }
    }

    /// Pre-play pull, no choice.
    pub fn pull_only() -> Self {
        Self {
            direction: SyncDirection::PullOnly,
            choice: None,
        }
    }

    /// Same direction with `choice`.
    pub fn with_choice(self, choice: Choice) -> Self {
        Self {
            choice: Some(choice),
            ..self
        }
    }
}

/// Everything a unit sync needs besides the unit.
pub struct SyncContext<'a> {
    pub provider: &'a dyn CloudStorageProvider,
    pub data_root: &'a Path,
    /// Base manifests of the signed-in account.
    pub bases: &'a BaseManifestStore,
    pub device: &'a DeviceIdentity,
    pub scope: SyncScope,
    pub progress: &'a SyncProgress,
}

/// How a unit sync ended (errors aside).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SyncStatus {
    /// Done; local and cloud agree (as far as the direction allows).
    #[default]
    Synced,
    /// Conflicts found and no choice given: nothing was written.
    NeedsChoice,
    /// Another PC kept committing this unit (two lost commits): try later.
    Busy,
    /// Blobs missing / corrupt in the cloud while the manifest did not move: unit not
    /// pulled (local untouched), paths flagged for repair in `index.json`.
    RemoteInconsistent { paths: Vec<String> },
    /// A local file changed while it was uploading (core writing in place): no
    /// commit; it is pushed again after the session.
    PushAborted { path: String },
}

/// What a unit sync did. Paths are relative to the unit folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncOutcome {
    pub status: SyncStatus,
    pub pushed: Vec<String>,
    pub pulled: Vec<String>,
    /// Removed locally (copy in backups).
    pub deleted_local: Vec<String>,
    /// Tombstoned in the cloud.
    pub deleted_remote: Vec<String>,
    pub conflicts: Vec<Conflict>,
    /// Bytes downloaded + uploaded.
    pub bytes: u64,
    pub skipped_large: Vec<String>,
    /// Local files a rename could not replace (held open): retried next sync.
    pub skipped_busy: Vec<String>,
    /// Moves / links that fell back to a byte copy (cross-volume, FAT).
    pub fallback_copies: usize,
    /// Generation committed by this sync, if any.
    pub committed_generation: Option<u64>,
}

/// Why a unit sync failed. Local saves are never left half-replaced.
#[derive(Debug, thiserror::Error)]
pub enum SyncUnitError {
    #[error(transparent)]
    Cloud(#[from] CloudError),
    #[error(transparent)]
    Local(#[from] CloudSyncError),
    /// [`SyncProgress::cancel`] was called during a transfer.
    #[error("cloud sync cancelled")]
    Cancelled,
}

impl SyncUnitError {
    /// Io error on `path`.
    pub(crate) fn io(path: &Path, err: std::io::Error) -> Self {
        Self::Local(CloudSyncError::io(path, err))
    }
}
