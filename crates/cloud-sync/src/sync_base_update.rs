//! The base written after a sync: the manifest just read (pull-only) or committed,
//! plus the local stat of every path whose local content is known to equal its
//! entry (pulled now, or snapshot sha = entry sha). Every other path gets no stat,
//! so the next snapshot re-hashes it; a pending local change then plans as a plain
//! push (its base entry = the remote entry).

use std::collections::BTreeMap;

use crate::base_manifest_store::{BaseManifest, LocalStat, RemoteBlobRef};
use crate::local_file_snapshot::LocalSnapshot;
use crate::sync_manifest::RemoteManifest;
use crate::sync_staging::is_safe_relative_path;

/// What the apply pass did to the local folder.
#[derive(Debug, Default)]
pub(crate) struct AppliedLocal {
    /// Pulled path → stat after the rename into place.
    pub(crate) pulled: BTreeMap<String, LocalStat>,
    pub(crate) deleted: Vec<String>,
    /// Paths a rename could not replace / remove (file held open): left as they were.
    pub(crate) skipped: Vec<String>,
    pub(crate) fallback_copies: usize,
}

impl AppliedLocal {
    /// Whether the local folder changed.
    pub(crate) fn changed_anything(&self) -> bool {
        !self.pulled.is_empty() || !self.deleted.is_empty()
    }
}

/// The base after a sync whose resulting cloud state is `manifest`.
///
/// Skipped paths keep their old base entry (or none) so the next sync pulls them
/// again; such a base then claims one generation below `manifest`, so the planner
/// still trusts it (remote newer) instead of reading the mixed refs as "not my
/// commit".
pub(crate) fn next_base(
    manifest: &RemoteManifest,
    snapshot: &LocalSnapshot,
    applied: &AppliedLocal,
    old: Option<&BaseManifest>,
    backup_remote_on_push: BTreeMap<String, RemoteBlobRef>,
) -> BaseManifest {
    let mut base = BaseManifest::new(manifest.clone());
    // Same view as the planner's remote: entries with unsafe paths are never based on.
    base.manifest
        .files
        .retain(|path, _| is_safe_relative_path(path));
    for path in &applied.skipped {
        match old.and_then(|old| old.manifest.files.get(path)) {
            Some(entry) => base.manifest.files.insert(path.clone(), entry.clone()),
            None => base.manifest.files.remove(path),
        };
    }
    if !applied.skipped.is_empty() {
        base.manifest.generation = manifest.generation.saturating_sub(1);
    }
    for (path, entry) in &base.manifest.files {
        let Some(sha) = entry.live_sha() else {
            continue;
        };
        if applied.skipped.contains(path) {
            continue;
        }
        let stat = applied.pulled.get(path).copied().or_else(|| {
            let file = snapshot.files.get(path)?;
            (file.sha256.as_deref() == Some(sha)).then_some(file.stat)
        });
        if let Some(stat) = stat {
            base.local.insert(path.clone(), stat);
        }
    }
    base.backup_remote_on_push = backup_remote_on_push;
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_file_snapshot::LocalFile;
    use crate::sync_manifest::FileEntry;

    fn entry(sha: &str, blob: &str) -> FileEntry {
        FileEntry {
            sha256: sha.into(),
            size: 1,
            modified_at: 0,
            blob: blob.into(),
            deleted_at: None,
        }
    }

    #[test]
    fn stats_only_for_known_equal_content_and_skipped_paths_keep_old_entries() {
        let mut manifest = RemoteManifest::empty("dev", "PC");
        manifest.generation = 4;
        for (path, sha) in [
            ("same", "s1"),
            ("pending", "r2"),
            ("pulled", "p3"),
            ("busy", "b4"),
        ] {
            manifest
                .files
                .insert(path.into(), entry(sha, &format!("blobs/4-dev/{path}")));
        }
        let stat = |n| LocalStat {
            size: n,
            modified_ms: n,
        };
        let mut snapshot = LocalSnapshot::default();
        for (path, sha, n) in [
            ("same", "s1", 1),
            ("pending", "local", 2),
            ("busy", "old", 3),
        ] {
            let file = LocalFile {
                stat: stat(n),
                sha256: Some(sha.into()),
            };
            snapshot.files.insert(path.into(), file);
        }
        let mut old = BaseManifest::new(RemoteManifest::empty("dev", "PC"));
        old.manifest
            .files
            .insert("busy".into(), entry("old", "blobs/1-dev/busy"));
        let applied = AppliedLocal {
            pulled: [("pulled".to_string(), stat(9))].into(),
            skipped: vec!["busy".into()],
            ..AppliedLocal::default()
        };
        let base = next_base(&manifest, &snapshot, &applied, Some(&old), BTreeMap::new());
        assert_eq!(base.local.get("same"), Some(&stat(1)));
        assert_eq!(base.local.get("pulled"), Some(&stat(9)));
        assert_eq!(base.local.get("pending"), None, "cleared: re-hash");
        assert_eq!(base.local.get("busy"), None);
        assert_eq!(
            base.live_sha("busy"),
            Some("old"),
            "skipped keeps old entry"
        );
        assert_eq!(base.manifest.generation, 3);
    }
}
