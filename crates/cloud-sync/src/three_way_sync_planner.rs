//! The pure three-way planner: (local snapshot, base, remote manifest) → actions +
//! conflicts. Total over its inputs: every combination of present / changed /
//! deleted / absent per side maps to exactly one outcome per path.
//!
//! Per file, against the base (B): local-only change → push (or tombstone when the
//! file was deleted); remote-only change → pull (or delete locally); both changed to
//! the same content → adopt; both changed differently → conflict. Without a valid
//! base every path has B = absent, which yields the first-sync rules: one side only
//! → push / pull, equal → adopt, different → conflict; nothing is ever tombstoned.

use std::collections::{BTreeMap, BTreeSet};

use crate::base_manifest_store::BaseManifest;
use crate::conflict_choice::Choice;
use crate::local_file_snapshot::LocalSnapshot;
use crate::sync_manifest::RemoteManifest;
use crate::sync_scope::SyncScope;
use crate::sync_scope_maps::{scope_maps, ScopedMaps};

/// Why a path needs the user's choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// Both sides changed (or were first created) with different content.
    BothChanged,
    /// Deleted on this PC, changed in the cloud.
    LocalDeletedRemoteChanged,
    /// Changed on this PC, deleted in the cloud.
    LocalChangedRemoteDeleted,
}

/// One conflicted path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
    pub kind: ConflictKind,
}

/// What a sync of one unit does. Paths are relative, `/`-separated.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncPlan {
    /// Upload as new blobs.
    pub push: Vec<String>,
    /// Download (the old local file goes to backups first).
    pub pull: Vec<String>,
    /// Remove locally (copy kept in backups).
    pub delete_local: Vec<String>,
    /// Tombstone remotely.
    pub delete_remote: Vec<String>,
    /// Both sides already agree: record in the base only.
    pub adopt: Vec<String>,
    pub conflicts: Vec<Conflict>,
    /// Nothing to write remotely (no push, no tombstone): no new commit needed.
    pub unchanged_remote: bool,
    /// Local files over the size cap (never transferred).
    pub skipped_large: Vec<String>,
    /// Paths whose action came from resolving a conflict, with the choice made.
    /// KeepLocal entries back up the cloud copy before upload; a `PullOnly` run
    /// defers their push without losing the choice.
    pub from_conflict: BTreeMap<String, Choice>,
}

impl SyncPlan {
    /// Recomputes [`SyncPlan::unchanged_remote`] after the lists changed.
    pub(crate) fn refresh_unchanged_remote(&mut self) {
        self.unchanged_remote = self.push.is_empty() && self.delete_remote.is_empty();
    }
}

/// The base the per-file rules may trust, or `None` for first-sync rules:
/// - no remote manifest (the cloud copy is gone) → `None`;
/// - whole unit gone (no in-scope local file while the base has some) → `None`:
///   a removed / emptied folder is never propagated as deletes;
/// - not my commit (`remote.generation <= base.generation` but the writer or any blob
///   ref differs: another PC won a last-writer-wins race) → `None`.
pub fn effective_base<'a>(
    maps: &ScopedMaps,
    base: Option<&'a BaseManifest>,
    remote: Option<&RemoteManifest>,
) -> Option<&'a BaseManifest> {
    let base = base?;
    let remote = remote?;
    if maps.local.is_empty() && !maps.base.is_empty() {
        return None;
    }
    if remote.generation <= base.manifest.generation && !remote.same_refs(&base.manifest) {
        return None;
    }
    Some(base)
}

/// Plans one unit. `remote` = `None` when the unit has no manifest yet.
pub fn plan_sync(
    local: &LocalSnapshot,
    base: Option<&BaseManifest>,
    remote: Option<&RemoteManifest>,
    scope: &SyncScope,
) -> SyncPlan {
    let mut maps = scope_maps(local, base, remote, scope);
    if effective_base(&maps, base, remote).is_none() {
        maps.forget_base();
    }
    let mut plan = SyncPlan {
        skipped_large: std::mem::take(&mut maps.skipped_large),
        ..SyncPlan::default()
    };
    let paths: BTreeSet<&String> = maps
        .local
        .keys()
        .chain(maps.base.keys())
        .chain(maps.remote.keys())
        .collect();
    for path in paths {
        let l = maps.local.get(path).map(String::as_str);
        let b = maps.base.get(path).map(String::as_str);
        let r = maps.remote.get(path).and_then(|sha| sha.as_deref());
        let list = match (l != b, r != b) {
            (false, false) => continue,
            (true, false) if l.is_some() => &mut plan.push,
            (true, false) => &mut plan.delete_remote,
            (false, true) if r.is_some() => &mut plan.pull,
            (false, true) => &mut plan.delete_local,
            (true, true) if l == r => &mut plan.adopt,
            (true, true) => {
                let kind = match (l, r) {
                    (None, _) => ConflictKind::LocalDeletedRemoteChanged,
                    (_, None) => ConflictKind::LocalChangedRemoteDeleted,
                    _ => ConflictKind::BothChanged,
                };
                plan.conflicts.push(Conflict {
                    path: path.clone(),
                    kind,
                });
                continue;
            }
        };
        list.push(path.clone());
    }
    plan.refresh_unchanged_remote();
    plan
}

#[cfg(test)]
#[path = "three_way_sync_planner_tests.rs"]
mod tests;
