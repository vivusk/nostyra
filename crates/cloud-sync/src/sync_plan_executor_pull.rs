//! Pull half of the executor: staged download + verification of every pulled file
//! (`Transferring`), then one backup-by-rename + rename-into-place pass
//! (`Applying`). A missing / corrupt blob aborts the pull before any local change:
//! the manifest is re-read once — moved (another PC committed, GC may have removed
//! blobs under this lagging reader) → restart; unchanged → `RemoteInconsistent` and
//! a `repair` flag on the unit's `index.json` entry.

use crate::cloud_index::{update_index, IndexEntry, RepairRef};
use crate::local_file_snapshot::stat_of;
use crate::local_save_backups::{prune_backups, BackupFolder, BackupSide};
use crate::sync_base_update::AppliedLocal;
use crate::sync_download::download_verified;
use crate::sync_outcome::{SyncStatus, SyncUnitError};
use crate::sync_plan_executor::{Step, UnitRun};
use crate::sync_progress::SyncPhase;
use crate::sync_remote_state::RemoteState;
use crate::sync_staging::{is_safe_relative_path, join_rel, move_file, FsOp};
use crate::three_way_sync_planner::SyncPlan;

impl UnitRun<'_, '_> {
    /// Downloads every pulled file into staging. `Some(broken)` = blobs missing or
    /// not matching their manifest sha (nothing applied).
    pub(crate) fn download(
        &mut self,
        remote: &RemoteState,
        plan: &SyncPlan,
    ) -> Result<Option<Vec<RepairRef>>, SyncUnitError> {
        let Some(manifest) = &remote.planned else {
            return Ok(None);
        };
        let mut broken = Vec::new();
        for path in &plan.pull {
            self.check_cancel()?;
            let Some(entry) = manifest.files.get(path) else {
                continue; // planned from this manifest: always present
            };
            let staged = self.staging.pull_path(path);
            let blob = format!("{}{}", self.prefix, entry.blob);
            let verified = if is_safe_relative_path(&entry.blob) {
                download_verified(self.ctx, &blob, &staged, &entry.sha256)?
            } else {
                None // user-edited manifest: never request outside the unit
            };
            match verified {
                Some(len) => self.outcome.bytes += len,
                None => broken.push(RepairRef {
                    path: path.clone(),
                    blob: entry.blob.clone(),
                }),
            }
        }
        Ok((!broken.is_empty()).then_some(broken))
    }

    /// Restart when the manifest moved, else report `RemoteInconsistent` and flag the
    /// broken entries for repair in `index.json` (best effort; not in `PullOnly`,
    /// which never writes to the cloud: the next `Both` run flags them).
    pub(crate) fn remote_inconsistent(
        &mut self,
        remote: &RemoteState,
        broken: Vec<RepairRef>,
        pull_only: bool,
    ) -> Result<Step, SyncUnitError> {
        if self.current_rev()? != remote.rev {
            return Ok(Step::Restart);
        }
        if !pull_only {
            self.flag_repair(remote, &broken);
        }
        let paths = broken.into_iter().map(|item| item.path).collect();
        tracing::warn!(target: "cloud_sync", "{}: cloud copy inconsistent: {paths:?}", self.key);
        self.outcome.status = SyncStatus::RemoteInconsistent { paths };
        Ok(Step::Done)
    }

    /// Adds `broken` to the unit's `repair` list in `index.json` (errors logged).
    fn flag_repair(&self, remote: &RemoteState, broken: &[RepairRef]) {
        let manifest = remote.planned.as_ref();
        let generation = manifest.map_or(0, |m| m.generation);
        let updated_at = manifest.map_or(0, |m| m.updated_at);
        let device_name = manifest.map(|m| m.device_name.clone()).unwrap_or_default();
        let result = update_index(self.ctx.provider, None, |index| {
            let entry = index
                .units
                .entry(self.key.clone())
                .or_insert_with(|| IndexEntry {
                    generation,
                    updated_at,
                    device_name: device_name.clone(),
                    repair: Vec::new(),
                });
            for item in broken {
                if !entry.repair.contains(item) {
                    entry.repair.push(item.clone());
                }
            }
        });
        if let Err(err) = result {
            tracing::warn!(target: "cloud_sync", "{}: cannot flag repair: {err}", self.key);
        }
    }

    /// Backs up replaced / deleted local files (rename into `<data>/cloud/backups`)
    /// and renames staged files into place (phase `Applying`; old backups are pruned
    /// after it, back in `Transferring`). A file that cannot be moved (held open) is
    /// left as it was and reported in `skipped`. `Cancelled` (nothing touched) when
    /// the job was cancelled before the pass could start.
    pub(crate) fn apply_pull(&mut self, plan: &SyncPlan) -> Result<AppliedLocal, SyncUnitError> {
        let mut applied = AppliedLocal::default();
        if plan.pull.is_empty() && plan.delete_local.is_empty() {
            return Ok(applied);
        }
        self.enter_applying()?;
        let dir = self.unit.local_dir().to_path_buf();
        let backup = BackupFolder::new(self.ctx.data_root, &self.key, BackupSide::Local);
        for path in &plan.pull {
            let target = join_rel(&dir, path);
            let backed_up = target.is_file();
            if backed_up {
                match backup.take(&target, path) {
                    Ok(op) => count(op, &mut applied),
                    Err(err) => {
                        self.skip(path, &err, &mut applied);
                        continue;
                    }
                }
            }
            match move_file(&self.staging.pull_path(path), &target) {
                Ok(op) => {
                    count(op, &mut applied);
                    let stat = std::fs::metadata(&target).map(|meta| stat_of(&meta));
                    applied
                        .pulled
                        .insert(path.clone(), stat.unwrap_or_default());
                }
                Err(err) => {
                    if backed_up {
                        let _ = move_file(&backup.path_of(path), &target); // put it back
                    }
                    self.skip(path, &err, &mut applied);
                }
            }
        }
        for path in &plan.delete_local {
            let target = join_rel(&dir, path);
            if !target.is_file() {
                continue;
            }
            match backup.take(&target, path) {
                Ok(op) => {
                    count(op, &mut applied);
                    applied.deleted.push(path.clone());
                }
                Err(err) => self.skip(path, &err, &mut applied),
            }
        }
        self.ctx.progress.set_phase(SyncPhase::Transferring);
        prune_backups(self.ctx.data_root, &self.key, backup.dir());
        self.outcome.pulled.extend(applied.pulled.keys().cloned());
        self.outcome
            .deleted_local
            .extend(applied.deleted.iter().cloned());
        self.outcome
            .skipped_busy
            .extend(applied.skipped.iter().cloned());
        self.outcome.fallback_copies += applied.fallback_copies;
        Ok(applied)
    }
}

/// Counts a copy fallback.
fn count(op: FsOp, applied: &mut AppliedLocal) {
    if op == FsOp::Copied {
        applied.fallback_copies += 1;
    }
}

impl UnitRun<'_, '_> {
    fn skip(&self, path: &str, err: &std::io::Error, applied: &mut AppliedLocal) {
        tracing::warn!(target: "cloud_sync", "{}: cannot replace {path}: {err}", self.key);
        applied.skipped.push(path.to_string());
    }
}
