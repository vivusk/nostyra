//! Blob GC across generations, a removed unit folder (r2 F1) and repair that would
//! lower content (r2 F4).

use crate::providers::fake_cloud_provider::{FakeCloudProvider, FakeOp};
use crate::sync_outcome::{SyncOptions, SyncStatus};
use crate::sync_test_harness::{local_folder, Pc};
use crate::{read_index, CloudStorageProvider, SyncScope};

#[test]
fn gc_keeps_previous_and_in_flight_dirs_and_removes_emptied_ones() {
    let fake = FakeCloudProvider::new();
    let a = Pc::new("PC-A");
    let ua = a.game("G");
    let dev8 = a.device.device8().to_string();
    let blobs = format!("{}blobs", ua.remote_prefix());
    a.write(&ua, "rtc.bin", b"r");
    for version in [b"v1", b"v2", b"v3"] {
        a.write(&ua, "save.srm", version);
        a.sync_ok(&fake, &ua);
    }
    let dirs = |fake: &FakeCloudProvider| -> Vec<String> {
        let mut names: Vec<String> = fake
            .list(&blobs)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        names.sort();
        names
    };
    let expect =
        |gens: &[u64]| -> Vec<String> { gens.iter().map(|g| format!("{g}-{dev8}")).collect() };
    assert_eq!(dirs(&fake), expect(&[1, 2, 3]), "gen 2 = previous manifest");
    fake.put(&format!("{blobs}/4-cccccccc/save.srm"), b"in flight");
    a.write(&ua, "save.srm", b"v4");
    fake.clear_calls();
    a.sync_ok(&fake, &ua);
    let mut want = expect(&[1, 3, 4]);
    want.push("4-cccccccc".to_string());
    want.sort();
    assert_eq!(dirs(&fake), want);
    let deleted: Vec<_> = fake
        .calls()
        .into_iter()
        .filter(|(op, _)| *op == FakeOp::DeleteDir)
        .collect();
    assert_eq!(
        deleted,
        vec![(FakeOp::DeleteDir, format!("{blobs}/2-{dev8}"))]
    );
}

#[test]
fn removed_unit_folder_pulls_everything_back() {
    let (_dir, cloud) = local_folder();
    let a = Pc::new("PC-A");
    let ua = a.game("G");
    a.write(&ua, "save.srm", b"s");
    a.write(&ua, "slot1.state", b"st");
    a.sync_ok(&cloud, &ua);
    std::fs::remove_dir_all(ua.local_dir()).unwrap();
    let back = a.sync_ok(&cloud, &ua);
    assert!(back.deleted_remote.is_empty());
    assert_eq!(back.pulled.len(), 2);
    assert_eq!(a.read(&ua, "slot1.state").unwrap(), b"st");
}

#[test]
fn repair_never_commits_from_a_pc_without_that_content() {
    let fake = FakeCloudProvider::new();
    let (a, mut d) = (Pc::new("PC-A"), Pc::new("PC-D"));
    let (ua, ud) = (a.game("G"), d.game("G"));
    a.write(&ua, "save.srm", b"s");
    a.write(&ua, "slot1.state", b"state v1");
    a.sync_ok(&fake, &ua);
    d.scope = SyncScope {
        sync_states: false,
        ..SyncScope::default()
    };
    d.sync_ok(&fake, &ud);
    d.write(&ud, "slot1.state", b"other content");
    let blob = fake
        .paths()
        .into_iter()
        .find(|p| p.ends_with("slot1.state"))
        .unwrap();
    fake.remove(&blob);
    let b = Pc::new("PC-B");
    let broken = b.sync(&fake, &b.game("G"), SyncOptions::both()).unwrap();
    assert!(matches!(
        broken.status,
        SyncStatus::RemoteInconsistent { .. }
    ));
    fake.clear_calls();
    d.sync_ok(&fake, &ud);
    assert!(!fake
        .calls()
        .iter()
        .any(|(op, _)| matches!(op, FakeOp::Write | FakeOp::WriteLarge)));
    let (index, _) = read_index(&fake).unwrap();
    assert_eq!(index.units[&ud.key()].repair.len(), 1, "flag kept");
}

#[test]
fn unsafe_manifest_paths_are_never_written_locally_but_carried_on_commit() {
    let fake = FakeCloudProvider::new();
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "save.srm", b"s1");
    a.sync_ok(&fake, &ua);
    let path = format!("{}manifest.json", ua.remote_prefix());
    let mut manifest = crate::RemoteManifest::parse(&fake.get(&path).unwrap()).unwrap();
    let evil = manifest.files["save.srm"].clone();
    manifest.files.insert("../../evil.srm".into(), evil);
    fake.put(&path, &manifest.encode(0).unwrap());

    b.sync_ok(&fake, &ub);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"s1");
    assert!(!b.root().join("saves").join("evil.srm").exists());
    b.write(&ub, "save.srm", b"s2");
    b.sync_ok(&fake, &ub);
    let committed = crate::RemoteManifest::parse(&fake.get(&path).unwrap()).unwrap();
    assert!(
        committed.files.contains_key("../../evil.srm"),
        "carried unchanged"
    );
    a.sync_ok(&fake, &ua); // base still valid: plain pull, no conflict
    assert_eq!(a.read(&ua, "save.srm").unwrap(), b"s2");
}
