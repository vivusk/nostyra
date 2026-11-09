//! The remote side of one unit attempt: the manifest as read (raw, and without
//! unsafe paths for planning) with its rev, and the cloud copies a "Keep this PC"
//! choice will overwrite (backed up before the push).

use std::collections::BTreeMap;

use crate::base_manifest_store::{BaseManifest, RemoteBlobRef};
use crate::cloud_error::CloudError;
use crate::cloud_storage_provider::{read_optional, Rev};
use crate::conflict_choice::Choice;
use crate::sync_manifest::{FileEntry, RemoteManifest};
use crate::sync_outcome::SyncUnitError;
use crate::sync_plan_executor::{UnitRun, MANIFEST_FILE};
use crate::sync_staging::is_safe_relative_path;
use crate::three_way_sync_planner::{Conflict, SyncPlan};

/// The remote manifest as read.
pub(crate) struct RemoteState {
    /// Exactly as stored (carried into the next commit).
    pub(crate) full: Option<RemoteManifest>,
    /// Without entries whose path is unsafe (never planned, pulled or based on).
    pub(crate) planned: Option<RemoteManifest>,
    pub(crate) rev: Option<Rev>,
}

impl UnitRun<'_, '_> {
    pub(crate) fn read_remote(&self) -> Result<RemoteState, SyncUnitError> {
        let path = format!("{}{MANIFEST_FILE}", self.prefix);
        let Some((bytes, rev)) = read_optional(self.ctx.provider, &path)? else {
            return Ok(RemoteState {
                full: None,
                planned: None,
                rev: None,
            });
        };
        let full = RemoteManifest::parse(&bytes).map_err(|err| match err {
            crate::CloudSyncError::UnsupportedRemoteFormat { .. } => {
                SyncUnitError::Cloud(CloudError::UnsupportedRemoteFormat)
            }
            other => SyncUnitError::Cloud(CloudError::Other(format!("invalid manifest: {other}"))),
        })?;
        let mut planned = full.clone();
        planned.files.retain(|path, _| {
            let safe = is_safe_relative_path(path);
            if !safe {
                tracing::warn!(target: "cloud_sync", "{}: ignoring unsafe path {path:?}", self.key);
            }
            safe
        });
        Ok(RemoteState {
            full: Some(full),
            planned: Some(planned),
            rev: Some(rev),
        })
    }

    /// The manifest's current rev (re-read after a missing / corrupt blob).
    pub(crate) fn current_rev(&self) -> Result<Option<Rev>, SyncUnitError> {
        let path = format!("{}{MANIFEST_FILE}", self.prefix);
        Ok(read_optional(self.ctx.provider, &path)?.map(|(_, rev)| rev))
    }
}

/// A unit's conflicts with the remote entry each was planned against.
pub(crate) type ConflictSet = Vec<(Conflict, Option<FileEntry>)>;

/// The conflicts of a plan with their remote entries: two sets are equal only when
/// the user would have seen exactly the same choice.
pub(crate) fn conflict_set(conflicts: &[Conflict], remote: Option<&RemoteManifest>) -> ConflictSet {
    conflicts
        .iter()
        .map(|conflict| {
            let entry = remote.and_then(|remote| remote.files.get(&conflict.path));
            (conflict.clone(), entry.cloned())
        })
        .collect()
}

/// Cloud copies a push will overwrite after "Keep this PC": this run's KeepLocal
/// paths (their current remote entry) plus flags left by a pre-play run, for paths
/// still about to be pushed / tombstoned.
pub(crate) fn cloud_backup_refs(
    plan: &SyncPlan,
    base: Option<&BaseManifest>,
    remote: Option<&RemoteManifest>,
) -> BTreeMap<String, RemoteBlobRef> {
    let pending = |path: &String| plan.push.contains(path) || plan.delete_remote.contains(path);
    let mut refs: BTreeMap<String, RemoteBlobRef> = base
        .map(|base| base.backup_remote_on_push.clone())
        .unwrap_or_default();
    refs.retain(|path, _| pending(path));
    for (path, choice) in &plan.from_conflict {
        let entry = remote.and_then(|remote| remote.files.get(path));
        if let (Choice::KeepLocal, Some(entry), true) = (choice, entry, pending(path)) {
            if let Some(sha) = entry.live_sha() {
                let blob = entry.blob.clone();
                refs.insert(
                    path.clone(),
                    RemoteBlobRef {
                        blob,
                        sha256: sha.to_string(),
                    },
                );
            }
        }
    }
    refs
}
