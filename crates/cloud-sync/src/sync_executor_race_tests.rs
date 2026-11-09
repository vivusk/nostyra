//! Races between PCs (F1, r2 F1 / F3 / F4 / F8): a push landing mid-commit, the
//! last-writer-wins model, a lagging reader under GC, blob GC, a removed unit
//! folder, repair that would lower content.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::conflict_choice::Choice;
use crate::providers::fake_cloud_provider::{FakeCloudProvider, FakeOp};
use crate::read_index;
use crate::sync_outcome::{SyncOptions, SyncStatus};
use crate::sync_test_harness::{local_folder, Pc};

/// Runs `action` once, on the first `op` whose path contains `needle`.
fn once_on(
    fake: &FakeCloudProvider,
    op: FakeOp,
    needle: &'static str,
    action: impl Fn() + Send + Sync + 'static,
) {
    let fired = AtomicBool::new(false);
    fake.set_hook(Some(Arc::new(move |seen, path| {
        if seen == op && path.contains(needle) && !fired.swap(true, Ordering::SeqCst) {
            action();
        }
    })));
}

/// A and B in sync on `save.srm` = "base"; both then change it locally.
fn diverged(fake: &FakeCloudProvider) -> (Pc, Arc<Pc>) {
    let (a, b) = (Pc::new("PC-A"), Arc::new(Pc::new("PC-B")));
    a.write(&a.game("G"), "save.srm", b"base");
    a.sync_ok(fake, &a.game("G"));
    b.sync_ok(fake, &b.game("G"));
    a.write(&a.game("G"), "save.srm", b"from A");
    b.write(&b.game("G"), "save.srm", b"from B");
    (a, b)
}

fn race(conditional: bool) {
    let fake = Arc::new(FakeCloudProvider::new());
    fake.set_conditional_writes(conditional);
    let (a, b) = diverged(&fake);
    let (fake2, b2) = (Arc::clone(&fake), Arc::clone(&b));
    once_on(&fake, FakeOp::Write, "manifest.json", move || {
        b2.sync_ok(&*fake2, &b2.game("G")); // B commits between A's blobs and manifest
    });
    let a_run = a.sync(&*fake, &a.game("G"), SyncOptions::both()).unwrap();
    fake.set_hook(None);
    let (winner, loser): (&[u8], &Pc) = if conditional {
        assert_eq!(a_run.status, SyncStatus::NeedsChoice, "A lost the rev race");
        (b"from B", &a)
    } else {
        assert_eq!(a_run.status, SyncStatus::Synced, "last writer wins");
        let b_next = b.sync(&*fake, &b.game("G"), SyncOptions::both()).unwrap();
        assert_eq!(
            b_next.status,
            SyncStatus::NeedsChoice,
            "not my commit → conflict"
        );
        (b"from A", &b)
    };
    let c = Pc::new("PC-C");
    c.sync_ok(&*fake, &c.game("G"));
    assert_eq!(
        c.read(&c.game("G"), "save.srm").unwrap(),
        winner,
        "committed version pullable"
    );
    loser.sync_choosing(&*fake, &loser.game("G"), Choice::UseRemote);
    assert_eq!(loser.read(&loser.game("G"), "save.srm").unwrap(), winner);
}

#[test]
fn push_mid_commit_with_conditional_writes() {
    race(true);
}

#[test]
fn push_mid_commit_without_conditional_writes() {
    race(false);
}

#[test]
fn desktop_client_overwrite_of_the_manifest_is_not_my_commit() {
    let (_dir, cloud) = local_folder();
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "save.srm", b"base");
    a.sync_ok(&cloud, &ua);
    b.sync_ok(&cloud, &ub);
    let manifest = cloud.root().join(ua.remote_prefix()).join("manifest.json");
    let gen1 = std::fs::read(&manifest).unwrap();
    b.write(&ub, "save.srm", b"from B");
    b.sync_ok(&cloud, &ub);
    let b_gen2 = std::fs::read(&manifest).unwrap();
    std::fs::write(&manifest, gen1).unwrap(); // A's client has not seen B's yet
    a.write(&ua, "save.srm", b"from A");
    a.sync_ok(&cloud, &ua);
    std::fs::write(&manifest, b_gen2).unwrap(); // the client then keeps B's copy
    let next = a.sync(&cloud, &ua, SyncOptions::both()).unwrap();
    assert_eq!(next.status, SyncStatus::NeedsChoice);
    assert_eq!(
        a.read(&ua, "save.srm").unwrap(),
        b"from A",
        "nothing overwritten"
    );
}

#[test]
fn lagging_reader_restarts_after_gc_removed_its_blobs() {
    let fake = Arc::new(FakeCloudProvider::new());
    let (a, c) = (Arc::new(Pc::new("PC-A")), Pc::new("PC-C"));
    a.write(&a.game("G"), "save.srm", b"v1");
    a.sync_ok(&*fake, &a.game("G"));
    let (fake2, a2) = (Arc::clone(&fake), Arc::clone(&a));
    once_on(&fake, FakeOp::Read, "/blobs/", move || {
        for version in [b"v2", b"v3"] {
            a2.write(&a2.game("G"), "save.srm", version);
            a2.sync_ok(&*fake2, &a2.game("G"));
        }
    });
    let pulled = c.sync(&*fake, &c.game("G"), SyncOptions::both()).unwrap();
    fake.set_hook(None);
    assert_eq!(pulled.status, SyncStatus::Synced);
    assert_eq!(c.read(&c.game("G"), "save.srm").unwrap(), b"v3");
    let (index, _) = read_index(&*fake).unwrap();
    assert!(index.units.values().all(|entry| entry.repair.is_empty()));
}

/// A restart after a lost race must not apply the user's choice to a conflict the
/// user never saw (another PC's commit raised a new one).
#[test]
fn restart_does_not_apply_the_choice_to_new_conflicts() {
    let fake = Arc::new(FakeCloudProvider::new());
    let (a, b) = (Pc::new("PC-A"), Arc::new(Pc::new("PC-B")));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "save.srm", b"base");
    a.write(&ua, "rtc.bin", b"r0");
    a.sync_ok(&*fake, &ua);
    b.sync_ok(&*fake, &ub);
    b.write(&ub, "save.srm", b"from B");
    b.sync_ok(&*fake, &ub);
    a.write(&ua, "save.srm", b"from A");
    a.write(&ua, "rtc.bin", b"rA");
    let shown = a.sync(&*fake, &ua, SyncOptions::both()).unwrap();
    assert_eq!(shown.status, SyncStatus::NeedsChoice);
    assert_eq!(shown.conflicts.len(), 1, "only save.srm conflicts so far");
    let (fake2, b2) = (Arc::clone(&fake), Arc::clone(&b));
    once_on(&fake, FakeOp::Write, "manifest.json", move || {
        b2.write(&b2.game("G"), "rtc.bin", b"rB"); // B commits a second path
        b2.sync_ok(&*fake2, &b2.game("G"));
    });
    let options = SyncOptions::both().with_choice(Choice::KeepLocal);
    let run = a.sync(&*fake, &ua, options).unwrap();
    fake.set_hook(None);
    assert_eq!(
        run.status,
        SyncStatus::NeedsChoice,
        "new conflict → ask again"
    );
    assert!(run.conflicts.iter().any(|c| c.path == "rtc.bin"));
    assert_eq!(a.read(&ua, "rtc.bin").unwrap(), b"rA", "local kept");
    let c = Pc::new("PC-C");
    c.sync_ok(&*fake, &c.game("G"));
    assert_eq!(
        c.read(&c.game("G"), "rtc.bin").unwrap(),
        b"rB",
        "cloud kept"
    );
}
