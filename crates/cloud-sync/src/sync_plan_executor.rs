//! Applies a [`crate::SyncPlan`] to one unit safely. Order per attempt:
//! 1. read the remote manifest (+ rev; absent = nothing committed yet);
//! 2. snapshot + plan; conflicts without a choice return **before any write**;
//! 3. pull: download every pulled file into staging and verify its sha
//!    (`Transferring`, cancellable), then back up + rename into place
//!    (`Applying`, ms-scale, not cancellable) — local data is never modified
//!    without a verified replacement or a backup;
//! 4. push (`Both` only): stage changed files by hard link, upload them as new
//!    immutable blobs, write the manifest last (conditional on the read rev);
//! 5. base, `index.json`, blob GC.
//!
//! A lost manifest race (`RevMismatch`, or blobs vanishing under a moved manifest)
//! restarts the unit once; a second loss ends as `Busy`.

use std::time::Instant;

use crate::conflict_choice::apply_choice;
use crate::local_file_snapshot::snapshot_local_dir;
use crate::remote_case_alias_filter::drop_case_aliases;
use crate::sync_base_update::next_base;
use crate::sync_outcome::{
    SyncContext, SyncDirection, SyncOptions, SyncOutcome, SyncStatus, SyncUnitError,
};
use crate::sync_progress::SyncPhase;
use crate::sync_remote_state::{cloud_backup_refs, conflict_set, ConflictSet};
use crate::sync_staging::UnitStaging;
use crate::sync_unit::SyncUnit;
use crate::three_way_sync_planner::plan_sync;

/// Restarts allowed after a lost race before the unit ends as `Busy`.
const MAX_RESTARTS: usize = 1;
/// Manifest file name inside a unit folder.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Syncs one unit. Never touches a unit a running game holds: the caller checks.
pub fn sync_unit(
    ctx: &SyncContext<'_>,
    unit: &SyncUnit,
    options: SyncOptions,
) -> Result<SyncOutcome, SyncUnitError> {
    let started = Instant::now();
    let mut outcome = SyncOutcome::default();
    let result = run_with_restart(ctx, unit, options, &mut outcome);
    ctx.progress.set_phase(SyncPhase::Idle);
    let ms = started.elapsed().as_millis();
    let key = unit.key();
    match &result {
        Ok(()) => tracing::info!(
            target: "cloud_sync",
            "{key}: {:?} pushed {} pulled {} deleted {}+{} conflicts {} bytes {} copies {} in {ms} ms",
            outcome.status, outcome.pushed.len(), outcome.pulled.len(),
            outcome.deleted_local.len(), outcome.deleted_remote.len(),
            outcome.conflicts.len(), outcome.bytes, outcome.fallback_copies
        ),
        Err(err) => tracing::warn!(target: "cloud_sync", "{key}: sync failed after {ms} ms: {err}"),
    }
    result.map(|()| outcome)
}

fn run_with_restart(
    ctx: &SyncContext<'_>,
    unit: &SyncUnit,
    options: SyncOptions,
    outcome: &mut SyncOutcome,
) -> Result<(), SyncUnitError> {
    let key = unit.key();
    let mut chosen = None; // conflicts the choice was given for (first attempt)
    for attempt in 0..=MAX_RESTARTS {
        let staging = UnitStaging::reset(ctx.data_root, &key)
            .map_err(|err| SyncUnitError::io(ctx.data_root, err))?;
        let mut run = UnitRun {
            ctx,
            unit,
            key: key.clone(),
            prefix: unit.remote_prefix(),
            staging: &staging,
            outcome: &mut *outcome,
        };
        let step = run.run(options, attempt == 0, &mut chosen);
        if let Err(err) = staging.discard() {
            tracing::warn!(target: "cloud_sync", "{key}: cannot clear staging: {err}");
        }
        match step? {
            Step::Done => return Ok(()),
            Step::Restart => {
                tracing::info!(target: "cloud_sync", "{key}: manifest moved, restarting")
            }
        }
    }
    outcome.status = SyncStatus::Busy;
    Ok(())
}

/// Whether an attempt finished or lost a race.
pub(crate) enum Step {
    Done,
    Restart,
}

/// One attempt at one unit.
pub(crate) struct UnitRun<'a, 'c> {
    pub(crate) ctx: &'a SyncContext<'c>,
    pub(crate) unit: &'a SyncUnit,
    pub(crate) key: String,
    /// `games/<sys>/<sha1>/` or `shared/<key>/`.
    pub(crate) prefix: String,
    pub(crate) staging: &'a UnitStaging,
    pub(crate) outcome: &'a mut SyncOutcome,
}

impl UnitRun<'_, '_> {
    /// `chosen` = the conflicts `options.choice` answered on the first attempt: after
    /// a restart (another PC just committed) the choice applies only to exactly
    /// those conflicts, anything new goes back to the user as `NeedsChoice`.
    fn run(
        &mut self,
        options: SyncOptions,
        first: bool,
        chosen: &mut Option<ConflictSet>,
    ) -> Result<Step, SyncUnitError> {
        self.ctx.progress.set_phase(SyncPhase::Transferring);
        self.check_cancel()?;
        let mut remote = self.read_remote()?;
        let base = self.ctx.bases.load(self.unit)?;
        let scope = &self.ctx.scope;
        let snapshot = snapshot_local_dir(self.unit.local_dir(), base.as_ref(), scope)?;
        if let Some(planned) = remote.planned.as_mut() {
            drop_case_aliases(planned, &snapshot, base.as_ref(), &self.key);
        }
        let mut plan = plan_sync(&snapshot, base.as_ref(), remote.planned.as_ref(), scope);
        self.outcome.skipped_large.clone_from(&plan.skipped_large);
        if !plan.conflicts.is_empty() {
            let set = conflict_set(&plan.conflicts, remote.planned.as_ref());
            let answered = first || chosen.as_ref() == Some(&set);
            let Some(choice) = options.choice.filter(|_| answered) else {
                self.outcome.conflicts = plan.conflicts;
                self.outcome.status = SyncStatus::NeedsChoice;
                return Ok(Step::Done);
            };
            *chosen = Some(set);
            plan = apply_choice(plan, choice);
        }
        let cloud_backups = cloud_backup_refs(&plan, base.as_ref(), remote.planned.as_ref());
        let pull_only = options.direction == SyncDirection::PullOnly;
        if let Some(broken) = self.download(&remote, &plan)? {
            return self.remote_inconsistent(&remote, broken, pull_only);
        }
        let applied = self.apply_pull(&plan)?;
        if let Some(manifest) = &remote.planned {
            if pull_only || applied.changed_anything() || !cloud_backups.is_empty() {
                let next = next_base(
                    manifest,
                    &snapshot,
                    &applied,
                    base.as_ref(),
                    cloud_backups.clone(),
                );
                self.ctx.bases.save(self.unit, &next)?;
            }
        }
        if pull_only {
            return Ok(Step::Done);
        }
        self.push(
            &remote,
            &plan,
            &snapshot,
            base.as_ref(),
            &applied,
            &cloud_backups,
        )
    }

    /// `Cancelled` once the job was asked to stop.
    pub(crate) fn check_cancel(&self) -> Result<(), SyncUnitError> {
        if self.ctx.progress.is_cancelled() {
            return Err(SyncUnitError::Cancelled);
        }
        Ok(())
    }

    /// Enters `Applying` (about to touch the save folder) unless cancelled: one atomic
    /// step, so a Play that cancelled a `Transferring` job never races an apply pass.
    pub(crate) fn enter_applying(&self) -> Result<(), SyncUnitError> {
        if self.ctx.progress.enter_applying() {
            Ok(())
        } else {
            Err(SyncUnitError::Cancelled)
        }
    }
}
