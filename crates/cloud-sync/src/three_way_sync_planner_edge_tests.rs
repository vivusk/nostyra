//! Base-validity, scope, conflict-choice and first-sync rows of the planner (the
//! per-file truth table and fixtures are in the parent test module).

use super::*;

#[test]
fn first_sync_without_remote_pushes_everything() {
    let plan = plan_sync(&snapshot(&[("save.srm", "a", 1)]), None, None, &scope());
    assert_eq!(plan.push, ["save.srm"]);
    assert!(!plan.unchanged_remote);
    let empty = plan_sync(&LocalSnapshot::default(), None, None, &scope());
    assert_eq!(
        empty,
        SyncPlan {
            unchanged_remote: true,
            ..SyncPlan::default()
        }
    );
}

fn full_unit(generation: u64, device: &str) -> RemoteManifest {
    let files = [
        ("save.srm", Some(("s", true)), 1),
        ("slot1.state", Some(("t", true)), 1),
    ];
    manifest(generation, device, &files)
}

#[test]
fn whole_unit_gone_pulls_and_never_tombstones() {
    let remote = full_unit(3, ME);
    let base = base(remote.clone());
    for local in [
        LocalSnapshot::default(),
        snapshot(&[("save.srm.tmp", "x", 1)]),
    ] {
        let plan = plan_sync(&local, Some(&base), Some(&remote), &scope());
        assert_eq!(plan.pull, ["save.srm", "slot1.state"]);
        assert!(
            plan.delete_remote.is_empty() && plan.conflicts.is_empty(),
            "{plan:?}"
        );
        assert!(plan.unchanged_remote);
    }
    // One file missing from a unit that still has files → a real deletion.
    let plan = plan_sync(
        &snapshot(&[("save.srm", "s", 1)]),
        Some(&base),
        Some(&remote),
        &scope(),
    );
    assert_eq!(plan.delete_remote, ["slot1.state"]);
}

#[test]
fn not_my_commit_plans_without_base() {
    let base = base(full_unit(3, ME));
    let local = snapshot(&[("save.srm", "s", 1), ("slot1.state", "t", 1)]);
    // Another PC won the race at the same / an older generation with other content.
    let mut raced = full_unit(3, OTHER);
    raced.files.get_mut("save.srm").unwrap().sha256 = "s2".into();
    let plan = plan_sync(&local, Some(&base), Some(&raced), &scope());
    assert_eq!(
        outcome(&plan, "save.srm"),
        Outcome::Conflict(ConflictKind::BothChanged)
    );
    assert_eq!(outcome(&plan, "slot1.state"), Outcome::Adopt);
    let mut older = full_unit(2, ME);
    older.files.get_mut("save.srm").unwrap().sha256 = "s2".into();
    let plan = plan_sync(&local, Some(&base), Some(&older), &scope());
    assert_eq!(
        outcome(&plan, "save.srm"),
        Outcome::Conflict(ConflictKind::BothChanged)
    );
    // Same generation, same writer and refs: my own commit → normal rules.
    let changed = snapshot(&[("save.srm", "s3", 1), ("slot1.state", "t", 1)]);
    let plan = plan_sync(&changed, Some(&base), Some(&base.manifest), &scope());
    assert_eq!(
        (plan.push.as_slice(), plan.conflicts.len()),
        (&["save.srm".to_string()][..], 0)
    );
}

#[test]
fn scope_applies_to_every_map() {
    // States off on PC B: states in base + remote, absent locally → left alone.
    let remote = full_unit(3, ME);
    let base_b = base(remote.clone());
    let off = SyncScope {
        sync_states: false,
        ..scope()
    };
    let plan = plan_sync(
        &snapshot(&[("save.srm", "s", 1)]),
        Some(&base_b),
        Some(&remote),
        &off,
    );
    assert_eq!(
        plan,
        SyncPlan {
            unchanged_remote: true,
            ..SyncPlan::default()
        }
    );

    // Local file grown past the cap with a base entry → unchanged, reported.
    let grown = snapshot(&[("save.srm", "s", 1), ("slot1.state", "", CAP + 1)]);
    let plan = plan_sync(&grown, Some(&base_b), Some(&remote), &scope());
    assert_eq!(outcome(&plan, "slot1.state"), Outcome::Nothing);
    assert_eq!(plan.skipped_large, ["slot1.state"]);

    // Remote-only oversize file → never pulled.
    let mut big = remote.clone();
    big.generation = 4;
    big.device_id = OTHER.into();
    big.files.insert(
        "huge.state".into(),
        FileEntry {
            size: CAP + 1,
            ..remote.files["save.srm"].clone()
        },
    );
    let plan = plan_sync(
        &snapshot(&[("save.srm", "s", 1), ("slot1.state", "t", 1)]),
        Some(&base_b),
        Some(&big),
        &scope(),
    );
    assert_eq!(outcome(&plan, "huge.state"), Outcome::Nothing);
}

#[test]
fn keep_local_tags_pushes_and_use_remote_pulls() {
    let base_m = manifest(
        1,
        ME,
        &[
            ("a", Some(("1", true)), 1),
            ("b", Some(("1", true)), 1),
            ("c", Some(("1", true)), 1),
        ],
    );
    let remote = manifest(
        2,
        OTHER,
        &[
            ("a", Some(("2", true)), 1),
            ("b", Some(("2", true)), 1),
            ("c", Some(("1", false)), 1),
        ],
    );
    let local = snapshot(&[("a", "3", 1), ("c", "3", 1)]);
    let plan = plan_sync(&local, Some(&base(base_m)), Some(&remote), &scope());
    assert_eq!(plan.conflicts.len(), 3);

    let keep = apply_choice(plan.clone(), Choice::KeepLocal);
    assert_eq!(
        (keep.push.as_slice(), keep.delete_remote.as_slice()),
        (
            &["a".to_string(), "c".to_string()][..],
            &["b".to_string()][..]
        )
    );
    assert!(keep.conflicts.is_empty() && !keep.unchanged_remote);
    assert!(
        keep.from_conflict.values().all(|c| *c == Choice::KeepLocal)
            && keep.from_conflict.len() == 3
    );

    let cloud = apply_choice(plan, Choice::UseRemote);
    assert_eq!(
        (cloud.pull.as_slice(), cloud.delete_local.as_slice()),
        (
            &["a".to_string(), "b".to_string()][..],
            &["c".to_string()][..]
        )
    );
    assert!(cloud.unchanged_remote && cloud.from_conflict["a"] == Choice::UseRemote);
}

#[test]
fn cleared_stat_with_remote_sha_and_changed_file_is_a_plain_push() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("save.srm"), b"new").unwrap();
    let old = crate::content_hash::sha256_hex(b"old");
    // KeepLocal before play: base := remote manifest, local stat cleared.
    let remote = manifest(2, OTHER, &[("save.srm", Some((old.as_str(), true)), 3)]);
    let base = base(remote.clone());
    let local = snapshot_local_dir(dir.path(), Some(&base), &scope()).unwrap();
    assert_eq!(local.hashed, 1);
    let plan = plan_sync(&local, Some(&base), Some(&remote), &scope());
    assert_eq!(plan.push, ["save.srm"]);
    assert!(plan.conflicts.is_empty());
}
