//! Push half of the executor (`Both` only): back up cloud copies a "Keep this PC"
//! choice overwrites, hard-link the changed files into staging (`Applying`), upload
//! each as a new immutable blob `blobs/<gen+1>-<device8>/<path>` while hashing the
//! streamed bytes (`Transferring`; streamed sha ≠ planned sha → a core wrote the
//! file in place: abort, no manifest), then write the manifest last, conditional on
//! the rev read at the start. Repairs (`index.json` `repair`) ride in the same
//! commit when this PC holds the exact content of the broken entry.

use std::collections::BTreeMap;

use crate::base_manifest_store::{BaseManifest, RemoteBlobRef};
use crate::cloud_error::CloudError;
use crate::cloud_index::read_index_or_reset;
use crate::local_file_snapshot::LocalSnapshot;
use crate::local_save_backups::{holds_backup, prune_backups, BackupFolder, BackupSide};
use crate::sync_base_update::{next_base, AppliedLocal};
use crate::sync_commit_finish::{repairable, unix_now};
use crate::sync_download::download_verified;
use crate::sync_manifest::{blob_path, FileEntry, RemoteManifest};
use crate::sync_outcome::{SyncStatus, SyncUnitError};
use crate::sync_plan_executor::{Step, UnitRun, MANIFEST_FILE};
use crate::sync_progress::SyncPhase;
use crate::sync_remote_state::RemoteState;
use crate::sync_staging::{is_safe_relative_path, join_rel, link_or_copy, FsOp};
use crate::sync_upload::upload_staged;
use crate::three_way_sync_planner::SyncPlan;

impl UnitRun<'_, '_> {
    /// Pushes, commits and finishes the unit (base, index, GC).
    pub(crate) fn push(
        &mut self,
        remote: &RemoteState,
        plan: &SyncPlan,
        snapshot: &LocalSnapshot,
        base: Option<&BaseManifest>,
        applied: &AppliedLocal,
        cloud_backups: &BTreeMap<String, RemoteBlobRef>,
    ) -> Result<Step, SyncUnitError> {
        let index = match read_index_or_reset(self.ctx.provider) {
            Ok(index) => Some(index),
            Err(err) if err.is_transient() || err == CloudError::AuthExpired => {
                return Err(err.into())
            }
            Err(err) => {
                tracing::warn!(target: "cloud_sync", "{}: index unusable: {err}", self.key);
                None
            }
        };
        let repairs = repairable(index.as_ref(), &self.key, remote.planned.as_ref(), snapshot);
        let mut uploads = plan.push.clone();
        uploads.extend(repairs.into_iter().filter(|path| !plan.push.contains(path)));
        if uploads.is_empty() && plan.delete_remote.is_empty() {
            if let Some(manifest) = &remote.planned {
                let next = next_base(manifest, snapshot, applied, base, BTreeMap::new());
                self.ctx.bases.save(self.unit, &next)?;
            }
            return Ok(Step::Done);
        }
        self.backup_cloud_copies(cloud_backups)?;
        self.enter_applying()?;
        let staged = self.stage(&uploads);
        self.ctx.progress.set_phase(SyncPhase::Transferring);
        if let Err(path) = staged {
            self.outcome.status = SyncStatus::PushAborted { path };
            return Ok(Step::Done);
        }
        let now = unix_now();
        let device = self.ctx.device;
        let generation = remote.planned.as_ref().map_or(0, |m| m.generation) + 1;
        let mut manifest = remote
            .full
            .clone()
            .unwrap_or_else(|| RemoteManifest::empty(&device.device_id, &device.device_name));
        for path in &uploads {
            self.check_cancel()?;
            let Some(file) = snapshot.files.get(path) else {
                continue; // planned from this snapshot: always present
            };
            let sha = file.sha256.clone().unwrap_or_default();
            let blob = blob_path(generation, &device.device_id, path);
            let remote_path = format!("{}{blob}", self.prefix);
            let staged = self.staging.push_path(path);
            let Some(size) = upload_staged(self.ctx, &staged, &remote_path, &sha)? else {
                tracing::warn!(target: "cloud_sync", "{}: {path} changed during upload", self.key);
                self.outcome.status = SyncStatus::PushAborted { path: path.clone() };
                return Ok(Step::Done);
            };
            self.outcome.bytes += size;
            let modified_at = file.stat.modified_ms / 1000;
            let entry = FileEntry {
                sha256: sha,
                size,
                modified_at,
                blob,
                deleted_at: None,
            };
            manifest.files.insert(path.clone(), entry);
        }
        for path in &plan.delete_remote {
            if let Some(entry) = manifest.files.get_mut(path) {
                entry.deleted_at = Some(now);
            }
        }
        manifest.generation = generation;
        manifest.updated_at = now;
        manifest.device_id.clone_from(&device.device_id);
        manifest.device_name.clone_from(&device.device_name);
        self.check_cancel()?;
        let bytes = manifest.encode(now)?;
        let path = format!("{}{MANIFEST_FILE}", self.prefix);
        match self.ctx.provider.write(&path, &bytes, remote.rev.as_ref()) {
            Ok(_) => {}
            Err(CloudError::RevMismatch) => return Ok(Step::Restart),
            Err(err) => return Err(err.into()),
        }
        self.outcome.committed_generation = Some(generation);
        self.outcome.pushed.clone_from(&plan.push);
        self.outcome.deleted_remote.clone_from(&plan.delete_remote);
        let next = next_base(&manifest, snapshot, applied, base, BTreeMap::new());
        self.ctx.bases.save(self.unit, &next)?;
        self.finish_commit(&manifest, remote, index, now);
        Ok(Step::Done)
    }

    /// Links every upload into staging; `Err(path)` = the file vanished.
    fn stage(&mut self, uploads: &[String]) -> Result<(), String> {
        for path in uploads {
            let local = join_rel(self.unit.local_dir(), path);
            match link_or_copy(&local, &self.staging.push_path(path)) {
                Ok(FsOp::Cheap) => {}
                Ok(FsOp::Copied) => self.outcome.fallback_copies += 1,
                Err(err) => {
                    tracing::warn!(target: "cloud_sync", "{}: cannot stage {path}: {err}", self.key);
                    return Err(path.clone());
                }
            }
        }
        Ok(())
    }

    /// Downloads the cloud copies "Keep this PC" overwrites into
    /// `backups/…-cloud/` (best effort: a blob already gone is logged). A copy
    /// already in a `-cloud` backup (push retried after a failure) is skipped.
    fn backup_cloud_copies(
        &mut self,
        refs: &BTreeMap<String, RemoteBlobRef>,
    ) -> Result<(), SyncUnitError> {
        let (root, side) = (self.ctx.data_root, BackupSide::Cloud);
        let folder = BackupFolder::new(root, &self.key, side);
        let mut wrote = false;
        for (path, item) in refs {
            self.check_cancel()?;
            if holds_backup(root, &self.key, side, path, &item.sha256) {
                continue;
            }
            let remote_path = format!("{}{}", self.prefix, item.blob);
            let dest = folder.path_of(path);
            let saved = if is_safe_relative_path(&item.blob) {
                download_verified(self.ctx, &remote_path, &dest, &item.sha256)?
            } else {
                None
            };
            match saved {
                Some(len) => {
                    self.outcome.bytes += len;
                    wrote = true;
                }
                None => {
                    tracing::warn!(target: "cloud_sync", "{}: cloud copy of {path} gone, no backup", self.key);
                }
            }
        }
        if wrote {
            prune_backups(root, &self.key, folder.dir());
        }
        Ok(())
    }
}
