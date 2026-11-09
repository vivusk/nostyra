//! The user's answer to a unit's conflicts ("Keep this PC" / "Use cloud") turned
//! into plain plan actions. Only conflicted paths follow the choice; the losing
//! side is backed up by the executor (every pull / local delete backs up the local
//! file; `from_conflict` = KeepLocal marks the cloud copy for a backup before upload).

use crate::three_way_sync_planner::{ConflictKind, SyncPlan};

/// Which side wins a unit's conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// This PC's files win: upload them (or tombstone a local deletion).
    KeepLocal,
    /// The cloud's files win: download them (or delete locally).
    UseRemote,
}

/// Resolves every conflict of `plan` with `choice`; the resulting actions are tagged
/// in `from_conflict`. A plan without conflicts is returned unchanged.
pub fn apply_choice(mut plan: SyncPlan, choice: Choice) -> SyncPlan {
    for conflict in std::mem::take(&mut plan.conflicts) {
        let list = match (choice, conflict.kind) {
            (Choice::KeepLocal, ConflictKind::LocalDeletedRemoteChanged) => &mut plan.delete_remote,
            (Choice::KeepLocal, _) => &mut plan.push,
            (Choice::UseRemote, ConflictKind::LocalChangedRemoteDeleted) => &mut plan.delete_local,
            (Choice::UseRemote, _) => &mut plan.pull,
        };
        list.push(conflict.path.clone());
        plan.from_conflict.insert(conflict.path, choice);
    }
    for list in [
        &mut plan.push,
        &mut plan.pull,
        &mut plan.delete_local,
        &mut plan.delete_remote,
    ] {
        list.sort();
    }
    plan.refresh_unchanged_remote();
    plan
}
