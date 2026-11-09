//! Pre-play `PullOnly` runs (r3 F1) and the `Applying` bound (r2 F2): big files
//! move by rename / hard link, an in-place write during upload aborts the push.

use std::io::{Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::conflict_choice::Choice;
use crate::providers::fake_cloud_provider::{FakeCloudProvider, FakeOp};
use crate::sync_outcome::{SyncOptions, SyncStatus};
use crate::sync_test_harness::Pc;

/// Big enough for the chunked `write_large` path; the rename / link checks do not
/// depend on size (a copy fallback would be counted at any size).
const BIG: usize = 100 * 1024 * 1024;

fn uploads(fake: &FakeCloudProvider) -> usize {
    let calls = fake.calls();
    calls
        .iter()
        .filter(|(op, _)| matches!(op, FakeOp::Write | FakeOp::WriteLarge))
        .count()
}

/// A and B in sync on `save.srm` + `rtc.bin`.
fn in_sync(fake: &FakeCloudProvider) -> (Pc, Pc) {
    let (a, b) = (Pc::new("PC-A"), Pc::new("PC-B"));
    a.write(&a.game("G"), "save.srm", b"s1");
    a.write(&a.game("G"), "rtc.bin", b"r1");
    a.sync_ok(fake, &a.game("G"));
    b.sync_ok(fake, &b.game("G"));
    (a, b)
}

#[test]
fn pull_only_pulls_and_leaves_pending_changes_for_the_next_full_sync() {
    let fake = FakeCloudProvider::new();
    let (a, b) = in_sync(&fake);
    let (ua, ub) = (a.game("G"), b.game("G"));
    b.write(&ub, "rtc.bin", b"r2");
    b.sync_ok(&fake, &ub);
    a.write(&ua, "save.srm", b"s2 pending");
    fake.clear_calls();
    let pulled = a.sync(&fake, &ua, SyncOptions::pull_only()).unwrap();
    assert_eq!(pulled.status, SyncStatus::Synced);
    assert_eq!(pulled.pulled, vec!["rtc.bin".to_string()]);
    assert_eq!(uploads(&fake), 0, "nothing uploaded before play");
    assert_eq!(a.read(&ua, "rtc.bin").unwrap(), b"r2");
    let base = a.base(&ua).unwrap();
    assert_eq!(base.manifest.generation, 2);
    assert!(
        !base.local.contains_key("save.srm"),
        "pending file re-hashed next time"
    );

    let pushed = a.sync_ok(&fake, &ua);
    assert_eq!(pushed.pushed, vec!["save.srm".to_string()]);
    b.sync_ok(&fake, &ub);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"s2 pending");
}

#[test]
fn keep_local_before_play_uploads_nothing_and_pushes_after_without_asking_again() {
    let fake = FakeCloudProvider::new();
    let (a, b) = in_sync(&fake);
    let (ua, ub) = (a.game("G"), b.game("G"));
    b.write(&ub, "save.srm", b"cloud copy");
    b.sync_ok(&fake, &ub);
    a.write(&ua, "save.srm", b"this PC");
    let asked = a.sync(&fake, &ua, SyncOptions::pull_only()).unwrap();
    assert_eq!(asked.status, SyncStatus::NeedsChoice);
    fake.clear_calls();
    let options = SyncOptions::pull_only().with_choice(Choice::KeepLocal);
    assert_eq!(
        a.sync(&fake, &ua, options).unwrap().status,
        SyncStatus::Synced
    );
    assert_eq!(uploads(&fake), 0);
    assert_eq!(a.read(&ua, "save.srm").unwrap(), b"this PC");
    let base = a.base(&ua).unwrap();
    let remote_sha = crate::sha256_hex(b"cloud copy");
    assert_eq!(
        base.live_sha("save.srm"),
        Some(remote_sha.as_str()),
        "base = remote entry"
    );
    assert_eq!(base.backup_remote_on_push["save.srm"].sha256, remote_sha);

    let after = a.sync_ok(&fake, &ua); // no choice given: must not conflict again
    assert_eq!(after.pushed, vec!["save.srm".to_string()]);
    assert_eq!(
        a.backup_of(&ua, "cloud", "save.srm").unwrap(),
        b"cloud copy"
    );
    assert!(
        a.base(&ua).unwrap().backup_remote_on_push.is_empty(),
        "flag cleared"
    );
    b.sync_ok(&fake, &ub);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"this PC");
}

#[test]
fn use_cloud_before_play_pulls_now_with_a_local_backup() {
    let fake = FakeCloudProvider::new();
    let (a, b) = in_sync(&fake);
    let (ua, ub) = (a.game("G"), b.game("G"));
    b.write(&ub, "save.srm", b"cloud copy");
    b.sync_ok(&fake, &ub);
    a.write(&ua, "save.srm", b"this PC");
    let options = SyncOptions::pull_only().with_choice(Choice::UseRemote);
    let used = a.sync(&fake, &ua, options).unwrap();
    assert_eq!(used.pulled, vec!["save.srm".to_string()]);
    assert_eq!(a.read(&ua, "save.srm").unwrap(), b"cloud copy");
    assert_eq!(a.backup_of(&ua, "local", "save.srm").unwrap(), b"this PC");
    assert!(a.sync_ok(&fake, &ua).pushed.is_empty());
}

#[test]
fn big_files_move_by_rename_and_hard_link() {
    let fake = FakeCloudProvider::new();
    let (a, b) = in_sync(&fake);
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "slot1.state", &vec![1u8; BIG]);
    let pushed = a.sync_ok(&fake, &ua);
    assert_eq!(pushed.fallback_copies, 0, "staging is a hard link");
    assert!(fake
        .calls()
        .iter()
        .any(|(op, p)| *op == FakeOp::WriteLarge && p.ends_with("slot1.state")));
    b.write(&ub, "slot1.state", &vec![2u8; BIG]);
    let asked = b.sync_choosing(&fake, &ub, Choice::UseRemote);
    assert_eq!(asked.fallback_copies, 0, "backup + place are renames");
    assert_eq!(b.backup_of(&ub, "local", "slot1.state").unwrap()[0], 2);
    assert_eq!(b.read(&ub, "slot1.state").unwrap()[0], 1);
}

#[test]
fn in_place_write_during_upload_aborts_the_push() {
    let fake = Arc::new(FakeCloudProvider::new());
    let (a, _b) = in_sync(&fake);
    let ua = a.game("G");
    a.write(&ua, "slot1.state", &vec![1u8; BIG]);
    let path = a.path(&ua, "slot1.state");
    let fired = AtomicBool::new(false);
    fake.set_hook(Some(Arc::new(move |op, _| {
        if op == FakeOp::WriteLarge && !fired.swap(true, Ordering::SeqCst) {
            let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            file.seek(SeekFrom::Start(10)).unwrap();
            file.write_all(b"core wrote here").unwrap(); // same file, no rename
        }
    })));
    let manifest = format!("{}manifest.json", ua.remote_prefix());
    let before = fake.get(&manifest);
    let aborted = a.sync(&*fake, &ua, crate::SyncOptions::both()).unwrap();
    fake.set_hook(None);
    assert_eq!(
        aborted.status,
        SyncStatus::PushAborted {
            path: "slot1.state".into()
        }
    );
    assert_eq!(fake.get(&manifest), before, "no manifest written");
    assert_eq!(
        a.sync_ok(&*fake, &ua).pushed,
        vec!["slot1.state".to_string()],
        "re-pushed later"
    );
}
