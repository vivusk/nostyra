//! After a commit: `index.json` bump (with repair flags cleared once fixed or
//! stale) and blob GC; plus which `repair` flags this PC can fix in its commit.

use crate::cloud_index::{update_index, CloudIndex, IndexEntry};
use crate::cloud_storage_provider::Rev;
use crate::local_file_snapshot::LocalSnapshot;
use crate::remote_blob_gc::collect_garbage;
use crate::sync_manifest::RemoteManifest;
use crate::sync_plan_executor::UnitRun;
use crate::sync_remote_state::RemoteState;

impl UnitRun<'_, '_> {
    /// After a commit: bump `index.json` (clearing fixed / stale repair flags) and
    /// collect unreferenced blob dirs. Failures are logged only.
    pub(crate) fn finish_commit(
        &self,
        manifest: &RemoteManifest,
        remote: &RemoteState,
        index: Option<(CloudIndex, Option<Rev>)>,
        now: u64,
    ) {
        if let Some(known) = index {
            let result = update_index(self.ctx.provider, Some(known), |idx| {
                let entry = idx
                    .units
                    .entry(self.key.clone())
                    .or_insert_with(|| IndexEntry {
                        generation: 0,
                        updated_at: 0,
                        device_name: String::new(),
                        repair: Vec::new(),
                    });
                entry.generation = manifest.generation;
                entry.updated_at = now;
                entry.device_name.clone_from(&manifest.device_name);
                entry.repair.retain(|item| {
                    let current = manifest.files.get(&item.path);
                    current.is_some_and(|f| f.blob == item.blob && f.deleted_at.is_none())
                });
            });
            if let Err(err) = result {
                tracing::warn!(target: "cloud_sync", "{}: index not updated: {err}", self.key);
            }
        }
        let previous = remote.full.as_ref();
        collect_garbage(self.ctx.provider, &self.prefix, manifest, previous);
    }
}

/// Repair entries this PC can fix: the current manifest still references the
/// broken blob and the local file has exactly that entry's content.
pub(crate) fn repairable(
    index: Option<&(CloudIndex, Option<Rev>)>,
    key: &str,
    manifest: Option<&RemoteManifest>,
    snapshot: &LocalSnapshot,
) -> Vec<String> {
    let (Some((index, _)), Some(manifest)) = (index, manifest) else {
        return Vec::new();
    };
    let Some(entry) = index.units.get(key) else {
        return Vec::new();
    };
    entry
        .repair
        .iter()
        .filter(|item| {
            let current = manifest.files.get(&item.path);
            let local = snapshot
                .files
                .get(&item.path)
                .and_then(|f| f.sha256.as_deref());
            current.is_some_and(|c| {
                c.blob == item.blob && c.live_sha().is_some() && c.live_sha() == local
            })
        })
        .map(|item| item.path.clone())
        .collect()
}

/// Seconds since the unix epoch (0 if the clock is before 1970).
pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |age| age.as_secs())
}
