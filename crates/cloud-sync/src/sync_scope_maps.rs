//! The planner's three inputs reduced to path → content maps, with the
//! [`SyncScope`] applied to all of them: a path out of scope on any side is dropped
//! everywhere (never pushed, pulled or tombstoned; the remote entry is carried
//! unchanged into the next manifest by the executor). One exception keeps the base
//! alive: a local file grown past the size cap whose base entry is in scope counts
//! as "unchanged vs base" (no tombstone) and is reported in `skipped_large`.

use std::collections::{BTreeMap, BTreeSet};

use crate::base_manifest_store::BaseManifest;
use crate::local_file_snapshot::LocalSnapshot;
use crate::sync_manifest::RemoteManifest;
use crate::sync_scope::SyncScope;

/// In-scope content per side.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopedMaps {
    /// Local path → sha256.
    pub local: BTreeMap<String, String>,
    /// Base path → sha256 (live entries only; a base tombstone = absent).
    pub base: BTreeMap<String, String>,
    /// Remote path → `Some(sha256)` live / `None` tombstone.
    pub remote: BTreeMap<String, Option<String>>,
    /// Local files over the size cap (never transferred), by path.
    pub skipped_large: Vec<String>,
    /// Over-cap local paths kept as "unchanged vs base".
    pub oversize_kept: BTreeSet<String>,
}

impl ScopedMaps {
    /// Drops the base (no-base planning). Over-cap paths that only stood on the base
    /// then leave every map: without a base nothing says they are unchanged.
    pub fn forget_base(&mut self) {
        self.base.clear();
        for path in std::mem::take(&mut self.oversize_kept) {
            self.local.remove(&path);
            self.remote.remove(&path);
        }
    }
}

/// Builds the scoped maps of one unit.
pub fn scope_maps(
    local: &LocalSnapshot,
    base: Option<&BaseManifest>,
    remote: Option<&RemoteManifest>,
    scope: &SyncScope,
) -> ScopedMaps {
    let mut maps = ScopedMaps::default();
    // Paths over the cap on some side (and not kept): untouched on every side.
    let mut dropped = BTreeSet::new();
    if let Some(base) = base {
        for (path, entry) in &base.manifest.files {
            match entry.live_sha() {
                Some(sha) if scope.in_scope(path, entry.size) => {
                    maps.base.insert(path.clone(), sha.to_string());
                }
                Some(_) if !scope.excluded_path(path) => {
                    dropped.insert(path.clone());
                }
                _ => {}
            }
        }
    }
    if let Some(remote) = remote {
        for (path, entry) in &remote.files {
            if scope.in_scope(path, entry.size) {
                maps.remote
                    .insert(path.clone(), entry.live_sha().map(str::to_string));
            } else if !scope.excluded_path(path) {
                dropped.insert(path.clone());
            }
        }
    }
    for (path, file) in &local.files {
        if scope.excluded_path(path) {
            continue;
        }
        if scope.over_cap(file.stat.size) {
            maps.skipped_large.push(path.clone());
            match maps.base.get(path) {
                Some(sha) => {
                    maps.local.insert(path.clone(), sha.clone());
                    maps.oversize_kept.insert(path.clone());
                }
                None => {
                    dropped.insert(path.clone());
                }
            }
        } else if let Some(sha) = &file.sha256 {
            maps.local.insert(path.clone(), sha.clone());
        }
    }
    for path in dropped.difference(&maps.oversize_kept) {
        maps.local.remove(path);
        maps.base.remove(path);
        maps.remote.remove(path);
    }
    maps
}
