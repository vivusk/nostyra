//! Cancellation at file / chunk boundaries: a cancelled push writes no manifest, a
//! cancelled pull leaves the local folder untouched; staging is always cleared.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::providers::fake_cloud_provider::{FakeCloudProvider, FakeOp};
use crate::sync_outcome::{SyncOptions, SyncUnitError};
use crate::sync_test_harness::Pc;

fn manifest_path(pc: &Pc) -> String {
    format!("{}manifest.json", pc.game("G").remote_prefix())
}

/// Cancels `pc`'s job on the first call of `op` touching a blob.
fn cancel_on(fake: &Arc<FakeCloudProvider>, pc: &Arc<Pc>, op: FakeOp) {
    let (pc, fired) = (Arc::clone(pc), AtomicBool::new(false));
    fake.set_hook(Some(Arc::new(move |seen, path| {
        if seen == op && path.contains("/blobs/") && !fired.swap(true, Ordering::SeqCst) {
            pc.progress.cancel();
        }
    })));
}

#[test]
fn cancel_mid_push_writes_no_manifest() {
    let fake = Arc::new(FakeCloudProvider::new());
    let a = Arc::new(Pc::new("PC-A"));
    let ua = a.game("G");
    a.write(&ua, "save.srm", b"v1");
    a.write(&ua, "rtc.bin", b"r1");
    cancel_on(&fake, &a, FakeOp::Write);
    let err = a.sync(&*fake, &ua, SyncOptions::both()).unwrap_err();
    assert!(matches!(err, SyncUnitError::Cancelled), "{err}");
    assert!(fake.get(&manifest_path(&a)).is_none());
    assert!(a.staging_empty(&ua));
    assert_eq!(a.base(&ua), None);
}

#[test]
fn cancel_mid_pull_leaves_local_untouched_and_no_staging() {
    let fake = Arc::new(FakeCloudProvider::new());
    let (a, b) = (Pc::new("PC-A"), Arc::new(Pc::new("PC-B")));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "save.srm", b"v1");
    a.write(&ua, "rtc.bin", b"r1");
    a.sync_ok(&*fake, &ua);
    b.sync_ok(&*fake, &ub);
    a.write(&ua, "save.srm", b"v2");
    a.write(&ua, "rtc.bin", b"r2");
    a.sync_ok(&*fake, &ua);
    let base_before = b.base(&ub);
    cancel_on(&fake, &b, FakeOp::Read);
    let err = b.sync(&*fake, &ub, SyncOptions::both()).unwrap_err();
    assert!(matches!(err, SyncUnitError::Cancelled), "{err}");
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"v1");
    assert_eq!(b.read(&ub, "rtc.bin").unwrap(), b"r1");
    assert!(b.staging_empty(&ub));
    assert!(b.backup_dirs(&ub).is_empty());
    assert_eq!(b.base(&ub), base_before);
}

/// A local file held open without sharing (antivirus / indexer) cannot be replaced:
/// it is skipped, left as it was, and pulled by the next sync without a conflict.
#[cfg(windows)]
#[test]
fn file_held_open_is_skipped_then_pulled_next_time() {
    use std::os::windows::fs::OpenOptionsExt;
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
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(b.path(&ub, "save.srm"))
        .unwrap();
    let first = b.sync_ok(&fake, &ub);
    assert_eq!(first.skipped_busy, vec!["save.srm".to_string()]);
    assert_eq!(first.pulled, vec!["rtc.bin".to_string()]);
    drop(held);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"v1", "left as it was");
    let second = b.sync_ok(&fake, &ub);
    assert_eq!(second.pulled, vec!["save.srm".to_string()]);
    assert_eq!(b.read(&ub, "save.srm").unwrap(), b"v2");
}

/// A big blob download stops mid-file (between chunks) on cancel: nothing applied,
/// no staging left.
#[test]
fn cancel_between_download_chunks_of_a_big_blob() {
    let fake = Arc::new(FakeCloudProvider::new());
    let (a, b) = (Pc::new("PC-A"), Arc::new(Pc::new("PC-B")));
    let (ua, ub) = (a.game("G"), b.game("G"));
    a.write(&ua, "slot1.state", &vec![7u8; 6 * 1024 * 1024]);
    a.sync_ok(&*fake, &ua);
    let (pc, fired) = (Arc::clone(&b), AtomicBool::new(false));
    fake.set_hook(Some(Arc::new(move |seen, _| {
        if seen == FakeOp::ReadChunk && !fired.swap(true, Ordering::SeqCst) {
            pc.progress.cancel(); // after the first 1 MiB chunk
        }
    })));
    let err = b.sync(&*fake, &ub, SyncOptions::both()).unwrap_err();
    fake.set_hook(None);
    assert!(matches!(err, SyncUnitError::Cancelled), "{err}");
    assert!(b.read(&ub, "slot1.state").is_none());
    assert!(b.staging_empty(&ub));
    assert_eq!(b.base(&ub), None);
}
