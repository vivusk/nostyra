//! Retries and hand-edited cloud folders: a failing push backs up the "Keep this PC"
//! cloud copy once, unsafe blob paths are never requested, a mangled manifest is a
//! cloud error, a mangled `index.json` is rebuilt, `PullOnly` never writes.

use crate::conflict_choice::Choice;
use crate::providers::fake_cloud_provider::{FakeCloudProvider, FakeOp};
use crate::sync_outcome::{SyncOptions, SyncStatus, SyncUnitError};
use crate::sync_test_harness::Pc;
use crate::{read_index, CloudError, RemoteManifest, INDEX_PATH};

fn manifest_path(pc: &Pc) -> String {
    format!("{}manifest.json", pc.game("G").remote_prefix())
}

/// A and B in sync on `save.srm`; B then commits "from B", A changes it locally.
fn diverged(fake: &FakeCloudProvider) -> (Pc, Pc) {
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    a.write(&a.game("G"), "save.srm", b"base");
    a.sync_ok(fake, &a.game("G"));
    b.sync_ok(fake, &b.game("G"));
    b.write(&b.game("G"), "save.srm", b"from B");
    b.sync_ok(fake, &b.game("G"));
    a.write(&a.game("G"), "save.srm", b"from A");
    (a, b)
}

#[test]
fn failing_keep_local_push_backs_up_the_cloud_copy_once() {
    let fake = FakeCloudProvider::new();
    let (a, _b) = diverged(&fake);
    let ua = a.game("G");
    fake.fail_next(Some(FakeOp::Write), "/blobs/", CloudError::QuotaFull, 2);
    let options = SyncOptions::both().with_choice(Choice::KeepLocal);
    let err = a.sync(&fake, &ua, options).unwrap_err();
    assert!(
        matches!(err, SyncUnitError::Cloud(CloudError::QuotaFull)),
        "{err}"
    );
    let err = a.sync(&fake, &ua, SyncOptions::both()).unwrap_err();
    assert!(
        matches!(err, SyncUnitError::Cloud(CloudError::QuotaFull)),
        "{err}"
    );
    a.sync_ok(&fake, &ua);
    let cloud_dirs = a
        .backup_dirs(&ua)
        .into_iter()
        .filter(|d| d.contains("-cloud"));
    assert_eq!(cloud_dirs.count(), 1, "{:?}", a.backup_dirs(&ua));
    assert_eq!(a.backup_of(&ua, "cloud", "save.srm").unwrap(), b"from B");
}

#[test]
fn unsafe_blob_path_is_inconsistent_and_never_requested() {
    let fake = FakeCloudProvider::new();
    let a = Pc::new("PC-A");
    a.write(&a.game("G"), "save.srm", b"v1");
    a.sync_ok(&fake, &a.game("G"));
    let path = manifest_path(&a);
    let mut manifest = RemoteManifest::parse(&fake.get(&path).unwrap()).unwrap();
    manifest.files.get_mut("save.srm").unwrap().blob = "../../x/save.srm".into();
    fake.put(&path, &manifest.encode(manifest.updated_at).unwrap());
    let b = Pc::new("PC-B");
    fake.clear_calls();
    let run = b.sync(&fake, &b.game("G"), SyncOptions::both()).unwrap();
    assert_eq!(
        run.status,
        SyncStatus::RemoteInconsistent {
            paths: vec!["save.srm".into()]
        }
    );
    assert!(fake.calls().iter().all(|(_, p)| !p.contains("..")));
}

#[test]
fn mangled_manifest_is_a_cloud_error_and_mangled_index_is_rebuilt() {
    let fake = FakeCloudProvider::new();
    let a = Pc::new("PC-A");
    let ua = a.game("G");
    a.write(&ua, "save.srm", b"v1");
    fake.put(INDEX_PATH, b"{ not json");
    a.sync_ok(&fake, &ua);
    let (index, _) = read_index(&fake).unwrap();
    assert_eq!(index.units[&ua.key()].generation, 1, "index rebuilt");

    fake.put(&manifest_path(&a), b"{ not json");
    let err = a.sync(&fake, &ua, SyncOptions::both()).unwrap_err();
    assert!(
        matches!(err, SyncUnitError::Cloud(CloudError::Other(_))),
        "{err}"
    );
    assert_eq!(fake.get(&manifest_path(&a)).unwrap(), b"{ not json");
}

#[test]
fn pull_only_never_writes_the_repair_flag() {
    let fake = FakeCloudProvider::new();
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    a.write(&a.game("G"), "save.srm", b"v1");
    a.sync_ok(&fake, &a.game("G"));
    let blob = fake
        .paths()
        .into_iter()
        .find(|p| p.contains("/blobs/"))
        .unwrap();
    assert!(fake.corrupt(&blob));
    fake.clear_calls();
    let run = b
        .sync(&fake, &b.game("G"), SyncOptions::pull_only())
        .unwrap();
    assert!(matches!(run.status, SyncStatus::RemoteInconsistent { .. }));
    let calls = fake.calls();
    assert!(calls
        .iter()
        .all(|(op, _)| !matches!(op, FakeOp::Write | FakeOp::WriteLarge)));
    let both = b.sync(&fake, &b.game("G"), SyncOptions::both()).unwrap();
    assert!(matches!(both.status, SyncStatus::RemoteInconsistent { .. }));
    let (index, _) = read_index(&fake).unwrap();
    assert_eq!(
        index.units[&b.game("G").key()].repair.len(),
        1,
        "flagged by Both"
    );
}
