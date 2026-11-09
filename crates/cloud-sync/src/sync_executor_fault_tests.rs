//! Executor fault injection on the fake provider: offline, interrupted commit,
//! lost rev races, corrupt blobs (+ repair), quota, cancellation.

use crate::providers::fake_cloud_provider::{FakeCloudProvider, FakeOp};
use crate::sync_outcome::{SyncOptions, SyncStatus, SyncUnitError};
use crate::sync_test_harness::Pc;
use crate::{read_index, CloudError};

fn manifest_path(pc: &Pc) -> String {
    format!("{}manifest.json", pc.game("G").remote_prefix())
}

fn generation(fake: &FakeCloudProvider, pc: &Pc) -> u64 {
    let bytes = fake.get(&manifest_path(pc)).unwrap();
    crate::RemoteManifest::parse(&bytes).unwrap().generation
}

#[test]
fn offline_or_throttled_at_read_changes_nothing() {
    let fake = FakeCloudProvider::new();
    let a = Pc::new("PC-A");
    let ua = a.game("G");
    a.write(&ua, "save.srm", b"local");
    fake.set_offline(true);
    let err = a.sync(&fake, &ua, SyncOptions::both()).unwrap_err();
    assert!(
        matches!(err, SyncUnitError::Cloud(CloudError::Offline)),
        "{err}"
    );
    fake.set_offline(false);
    fake.fail_nth_call(1, CloudError::RateLimited { retry_after: None });
    let err = a.sync(&fake, &ua, SyncOptions::both()).unwrap_err();
    assert!(matches!(err, SyncUnitError::Cloud(e) if e.is_transient()));
    assert!(fake.paths().is_empty());
    assert_eq!(a.base(&ua), None);
    assert!(a.staging_empty(&ua));
    assert_eq!(a.read(&ua, "save.srm").unwrap(), b"local");
}

#[test]
fn crash_before_manifest_keeps_the_committed_generation_and_retry_pushes_once() {
    let fake = FakeCloudProvider::new();
    let (a, c) = (Pc::new("PC-A"), Pc::new("PC-C"));
    let (ua, uc) = (a.game("G"), c.game("G"));
    a.write(&ua, "save.srm", b"v1");
    a.sync_ok(&fake, &ua);
    a.write(&ua, "save.srm", b"v2");
    fake.fail_next(
        Some(FakeOp::Write),
        "manifest.json",
        CloudError::Other("killed".into()),
        1,
    );
    assert!(a.sync(&fake, &ua, SyncOptions::both()).is_err());
    assert!(
        fake.paths().iter().any(|p| p.contains("/blobs/2-")),
        "orphan blob uploaded"
    );
    assert_eq!(generation(&fake, &a), 1);

    c.sync_ok(&fake, &uc);
    assert_eq!(
        c.read(&uc, "save.srm").unwrap(),
        b"v1",
        "committed version intact"
    );

    let retry = a.sync_ok(&fake, &ua);
    assert_eq!(
        retry.committed_generation,
        Some(2),
        "no duplicate generation"
    );
    c.sync_ok(&fake, &uc);
    assert_eq!(c.read(&uc, "save.srm").unwrap(), b"v2");
}

#[test]
fn one_lost_race_restarts_two_end_busy() {
    let fake = FakeCloudProvider::new();
    let a = Pc::new("PC-A");
    let ua = a.game("G");
    a.write(&ua, "save.srm", b"v1");
    fake.rev_mismatch_once("manifest.json");
    assert_eq!(a.sync_ok(&fake, &ua).committed_generation, Some(1));

    a.write(&ua, "save.srm", b"v2");
    fake.fail_next(
        Some(FakeOp::Write),
        "manifest.json",
        CloudError::RevMismatch,
        2,
    );
    let busy = a.sync(&fake, &ua, SyncOptions::both()).unwrap();
    assert_eq!(busy.status, SyncStatus::Busy);
    assert_eq!(generation(&fake, &a), 1);
    assert_eq!(a.sync_ok(&fake, &ua).committed_generation, Some(2));
}

#[test]
fn corrupt_blob_aborts_the_pull_and_a_pc_with_the_content_repairs_it() {
    let fake = FakeCloudProvider::new();
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "save.srm", b"v1");
    a.write(&ua, "rtc.bin", b"r1");
    a.sync_ok(&fake, &ua);
    b.sync_ok(&fake, &ub);
    a.write(&ua, "save.srm", b"v2");
    a.write(&ua, "rtc.bin", b"r2");
    a.sync_ok(&fake, &ua);
    let blob = fake
        .paths()
        .into_iter()
        .find(|p| p.contains("/blobs/2-") && p.ends_with("save.srm"))
        .unwrap();
    assert!(fake.corrupt(&blob));

    let broken = b.sync(&fake, &ub, SyncOptions::both()).unwrap();
    assert_eq!(
        broken.status,
        SyncStatus::RemoteInconsistent {
            paths: vec!["save.srm".into()]
        }
    );
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"v1", "local untouched");
    assert_eq!(b.read(&ub, "rtc.bin").unwrap(), b"r1", "nothing applied");
    let (index, _) = read_index(&fake).unwrap();
    assert_eq!(index.units[&ub.key()].repair.len(), 1);

    let repaired = a.sync_ok(&fake, &ua);
    assert_eq!(repaired.committed_generation, Some(3), "repair-only commit");
    assert!(repaired.pushed.is_empty());
    let (index, _) = read_index(&fake).unwrap();
    assert!(index.units[&ua.key()].repair.is_empty(), "flag cleared");
    b.sync_ok(&fake, &ub);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"v2");
    assert_eq!(b.read(&ub, "rtc.bin").unwrap(), b"r2");
}

#[test]
fn quota_full_fails_the_unit_and_keeps_the_base() {
    let fake = FakeCloudProvider::new();
    let a = Pc::new("PC-A");
    let ua = a.game("G");
    a.write(&ua, "save.srm", b"v1");
    a.sync_ok(&fake, &ua);
    let before = a.base(&ua);
    a.write(&ua, "save.srm", b"v2");
    fake.fail_next(Some(FakeOp::Write), "/blobs/", CloudError::QuotaFull, 1);
    let err = a.sync(&fake, &ua, SyncOptions::both()).unwrap_err();
    assert!(
        matches!(err, SyncUnitError::Cloud(CloudError::QuotaFull)),
        "{err}"
    );
    assert_eq!(a.base(&ua), before);
    assert_eq!(generation(&fake, &a), 1);
}
