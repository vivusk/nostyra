use super::*;
use crate::base_manifest_store::LocalStat;
use crate::conflict_choice::apply_choice;
use crate::local_file_snapshot::{snapshot_local_dir, LocalFile};
use crate::sync_manifest::FileEntry;

/// A side's file: `None` = absent, `Some((sha, live))`, `live = false` = tombstone.
type Side<'a> = Option<(&'a str, bool)>;

const ME: &str = "aaaaaaaa11111111";
const OTHER: &str = "bbbbbbbb22222222";
const CAP: u64 = 100;

fn scope() -> SyncScope {
    SyncScope {
        sync_states: true,
        max_file_bytes: CAP,
    }
}

fn snapshot(files: &[(&str, &str, u64)]) -> LocalSnapshot {
    let files = files.iter().map(|&(path, sha, size)| {
        let stat = LocalStat {
            size,
            modified_ms: 1,
        };
        let sha256 = (size <= CAP).then(|| sha.to_string());
        (path.to_string(), LocalFile { stat, sha256 })
    });
    LocalSnapshot {
        files: files.collect(),
        hashed: 0,
    }
}

fn manifest(generation: u64, device: &str, files: &[(&str, Side, u64)]) -> RemoteManifest {
    let mut manifest = RemoteManifest::empty(device, "PC");
    manifest.generation = generation;
    for &(path, side, size) in files {
        let Some((sha, live)) = side else { continue };
        let entry = FileEntry {
            sha256: sha.to_string(),
            size,
            modified_at: 0,
            blob: format!("blobs/{generation}-{}/{path}", &device[..8]),
            deleted_at: (!live).then_some(5),
        };
        manifest.files.insert(path.to_string(), entry);
    }
    manifest
}

fn base(manifest: RemoteManifest) -> BaseManifest {
    BaseManifest::new(manifest)
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Nothing,
    Push,
    Pull,
    DeleteLocal,
    DeleteRemote,
    Adopt,
    Conflict(ConflictKind),
}

fn outcome(plan: &SyncPlan, path: &str) -> Outcome {
    let has = |list: &[String]| list.iter().any(|p| p == path);
    let found = [
        (has(&plan.push), Outcome::Push),
        (has(&plan.pull), Outcome::Pull),
        (has(&plan.delete_local), Outcome::DeleteLocal),
        (has(&plan.delete_remote), Outcome::DeleteRemote),
        (has(&plan.adopt), Outcome::Adopt),
    ];
    let mut hits: Vec<Outcome> = found
        .into_iter()
        .filter(|(hit, _)| *hit)
        .map(|(_, o)| o)
        .collect();
    hits.extend(
        plan.conflicts
            .iter()
            .filter(|c| c.path == path)
            .map(|c| Outcome::Conflict(c.kind)),
    );
    assert!(hits.len() <= 1, "{path}: several outcomes {hits:?}");
    hits.pop().unwrap_or(Outcome::Nothing)
}

/// Plans `save.srm` with (local, base, remote) beside an untouched `keep.srm` (so
/// the unit is never "gone"). Base at gen 1 by this PC; remote = base when equal,
/// else gen 2 by another PC. `base = None` → no base file at all.
fn plan_row(local: Option<&str>, base_side: Option<Side>, remote: Side) -> Outcome {
    let keep = ("keep.srm", Some(("k", true)), 1);
    let mut local_files = vec![("keep.srm", "k", 1)];
    local_files.extend(local.map(|sha| ("save.srm", sha, 1)));
    let base_manifest = base_side.map(|b| manifest(1, ME, &[keep, ("save.srm", b, 1)]));
    let remote_manifest = match &base_manifest {
        Some(b)
            if b.files
                .get("save.srm")
                .map(|e| (e.sha256.as_str(), e.deleted_at.is_none()))
                == remote =>
        {
            b.clone()
        }
        _ => manifest(2, OTHER, &[keep, ("save.srm", remote, 1)]),
    };
    let base = base_manifest.map(base);
    let plan = plan_sync(
        &snapshot(&local_files),
        base.as_ref(),
        Some(&remote_manifest),
        &scope(),
    );
    assert_eq!(
        outcome(&plan, "keep.srm"),
        if base.is_some() {
            Outcome::Nothing
        } else {
            Outcome::Adopt
        }
    );
    outcome(&plan, "save.srm")
}

#[test]
fn three_way_truth_table() {
    use ConflictKind::*;
    use Outcome::*;
    let a = Some(("a", true));
    let b = Some(("b", true));
    let c = Some(("c", true));
    let a_gone = Some(("a", false));
    let b_gone = Some(("b", false));
    let rows: [(Option<&str>, Option<Side>, Side, Outcome); 21] = [
        // Base = a.
        (Some("a"), Some(a), a, Nothing),
        (Some("b"), Some(a), a, Push),
        (None, Some(a), a, DeleteRemote),
        (Some("a"), Some(a), b, Pull),
        (Some("a"), Some(a), a_gone, DeleteLocal),
        (Some("a"), Some(a), None, DeleteLocal),
        (Some("b"), Some(a), b, Adopt),
        (Some("b"), Some(a), c, Conflict(BothChanged)),
        (None, Some(a), b, Conflict(LocalDeletedRemoteChanged)),
        (
            Some("b"),
            Some(a),
            a_gone,
            Conflict(LocalChangedRemoteDeleted),
        ),
        (
            Some("b"),
            Some(a),
            None,
            Conflict(LocalChangedRemoteDeleted),
        ),
        (None, Some(a), a_gone, Adopt),
        (None, Some(a), None, Adopt),
        // Base holds a tombstone / no entry for the file.
        (Some("a"), Some(a_gone), a_gone, Push),
        (None, Some(a_gone), a, Pull),
        (Some("a"), Some(None), b_gone, Push),
        (Some("a"), Some(None), a, Adopt),
        (Some("a"), Some(None), b, Conflict(BothChanged)),
        (None, Some(None), b_gone, Nothing),
        // First sync on this PC (no base file).
        (Some("a"), None, b, Conflict(BothChanged)),
        (Some("a"), None, a, Adopt),
    ];
    for (local, base_side, remote, want) in rows {
        assert_eq!(
            plan_row(local, base_side, remote),
            want,
            "{local:?} {base_side:?} {remote:?}"
        );
    }
}

#[path = "three_way_sync_planner_edge_tests.rs"]
mod edge;
