//! Planner input sanitising against case aliases: on Windows `SAVE.SRM` and
//! `save.srm` are the same file, so a remote entry that differs from a local, base
//! or other remote path only by ASCII case would be pulled straight over that file
//! (no conflict prompt). Such remote entries are dropped from the planned manifest
//! (logged), exactly like unsafe paths; the stored manifest keeps them.

use std::collections::BTreeSet;

use crate::base_manifest_store::BaseManifest;
use crate::local_file_snapshot::LocalSnapshot;
use crate::sync_manifest::RemoteManifest;

/// Drops live remote entries that are new to this PC (not a local / base path) yet
/// equal another local, base or live remote path ignoring ASCII case. Entries the
/// PC already knows by their exact path are kept, so a drop never turns into a
/// local delete. Tombstones are kept (never pulled).
pub(crate) fn drop_case_aliases(
    remote: &mut RemoteManifest,
    local: &LocalSnapshot,
    base: Option<&BaseManifest>,
    unit_key: &str,
) {
    let known: BTreeSet<&String> = local
        .files
        .keys()
        .chain(base.into_iter().flat_map(|b| b.manifest.files.keys()))
        .chain(base.into_iter().flat_map(|b| b.local.keys()))
        .collect();
    let live: Vec<&String> = remote
        .files
        .iter()
        .filter(|(_, entry)| entry.deleted_at.is_none())
        .map(|(path, _)| path)
        .collect();
    let aliased: Vec<String> = live
        .iter()
        .filter(|path| !known.contains(*path))
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            let other = |p: &&String| p != *path && p.to_ascii_lowercase() == lower;
            known.iter().any(other) || live.iter().any(other)
        })
        .map(|path| (*path).clone())
        .collect();
    for path in aliased {
        tracing::warn!(target: "cloud_sync", "{unit_key}: ignoring case alias {path:?}");
        remote.files.remove(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base_manifest_store::LocalStat;
    use crate::local_file_snapshot::LocalFile;
    use crate::sync_manifest::FileEntry;

    fn entry(deleted: bool) -> FileEntry {
        FileEntry {
            sha256: "ab".repeat(32),
            size: 1,
            modified_at: 0,
            blob: "blobs/1-aaaaaaaa/x".into(),
            deleted_at: deleted.then_some(1),
        }
    }

    fn manifest(paths: &[(&str, bool)]) -> RemoteManifest {
        let mut m = RemoteManifest::empty("dev", "pc");
        for (path, deleted) in paths {
            m.files.insert((*path).into(), entry(*deleted));
        }
        m
    }

    fn snapshot(paths: &[&str]) -> LocalSnapshot {
        let mut s = LocalSnapshot::default();
        for path in paths {
            let stat = LocalStat {
                size: 1,
                modified_ms: 0,
            };
            s.files
                .insert((*path).into(), LocalFile { stat, sha256: None });
        }
        s
    }

    fn keys(m: &RemoteManifest) -> Vec<&str> {
        m.files.keys().map(String::as_str).collect()
    }

    #[test]
    fn remote_only_alias_of_a_local_file_is_dropped() {
        let mut remote = manifest(&[("SAVE.SRM", false), ("other.srm", false)]);
        drop_case_aliases(&mut remote, &snapshot(&["save.srm"]), None, "u");
        assert_eq!(keys(&remote), ["other.srm"]);
    }

    #[test]
    fn alias_of_a_base_path_is_dropped_exact_path_kept() {
        let base = BaseManifest::new(manifest(&[("Card/a.mcd", false)]));
        let mut remote = manifest(&[("Card/a.mcd", false), ("card/A.MCD", false)]);
        drop_case_aliases(&mut remote, &snapshot(&[]), Some(&base), "u");
        assert_eq!(keys(&remote), ["Card/a.mcd"]);
    }

    #[test]
    fn two_new_remote_paths_equal_ignoring_case_are_both_dropped() {
        let mut remote = manifest(&[("a.srm", false), ("A.srm", false), ("b.srm", false)]);
        drop_case_aliases(&mut remote, &snapshot(&[]), None, "u");
        assert_eq!(keys(&remote), ["b.srm"]);
    }

    #[test]
    fn exact_paths_and_tombstones_are_kept() {
        let mut remote = manifest(&[("save.srm", false), ("SAVE.srm", true)]);
        drop_case_aliases(&mut remote, &snapshot(&["save.srm"]), None, "u");
        assert_eq!(keys(&remote), ["SAVE.srm", "save.srm"]);
    }
}
