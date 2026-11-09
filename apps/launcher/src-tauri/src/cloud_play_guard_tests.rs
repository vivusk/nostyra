//! Play vs. a cloud job (F3, r2 F2): offline and slow clouds never delay Play past
//! the budget, a transfer is cancelled (a push never commits, a pull writes nothing
//! after the runner start), an apply pass is waited for.

use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime};

use cloud_sync::providers::fake_cloud_provider::{FakeCloudProvider, FakeOp};
use cloud_sync::{SyncOptions, SyncPhase};

use crate::cloud_game_result::SyncGameResult;
use crate::cloud_sync_jobs::run_within_budget;
use crate::cloud_sync_state::JobKind;
use crate::cloud_test_harness::{fake_account, generation, TestHost, SHA1_A, SHA1_B, SYSTEM};
use crate::cloud_unit_guard::{clear_units_for_play, session_unit_keys, PlayGuard};

fn stop() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

/// Delays every blob call of `op` by `delay`; `seen` fires on the first one.
fn slow_blobs(fake: &FakeCloudProvider, op: FakeOp, delay: Duration) -> mpsc::Receiver<()> {
    let (seen, receiver) = mpsc::channel();
    let seen = std::sync::Mutex::new(Some(seen));
    fake.set_hook(Some(Arc::new(move |call, path| {
        if call == op && path.contains("/blobs/") {
            if let Some(seen) = seen.lock().unwrap().take() {
                let _ = seen.send(());
            }
            std::thread::sleep(delay);
        }
    })));
    receiver
}

#[test]
fn offline_cloud_answers_offline_at_once_and_keeps_local_saves() {
    let fake = Arc::new(FakeCloudProvider::new());
    fake.set_offline(true);
    let b = TestHost::new("PC-B");
    let tb = b.game(SYSTEM, SHA1_A);
    b.write(&tb, "save.srm", b"local");
    let started = Instant::now();
    let result = b.pre_play(&fake_account(&fake), &tb, SyncOptions::pull_only());
    assert_eq!(result, SyncGameResult::Offline);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(b.read(&tb, "save.srm").unwrap(), b"local");
}

#[test]
fn budget_expiry_cancels_a_slow_pull_and_leaves_the_folder_untouched() {
    let fake = Arc::new(FakeCloudProvider::new());
    let account = fake_account(&fake);
    let (a, b) = (TestHost::new("PC-A"), TestHost::new("PC-B"));
    let (ta, tb) = (a.game(SYSTEM, SHA1_A), b.game(SYSTEM, SHA1_A));
    for name in ["save.srm", "rtc.bin", "slot1.state"] {
        a.write(&ta, name, name.as_bytes());
    }
    a.sync_both(&account, &ta);
    let _seen = slow_blobs(&fake, FakeOp::Read, Duration::from_millis(300));
    let started = Instant::now();
    let request = b.request(&account, &tb, SyncOptions::pull_only());
    let result = run_within_budget(&b, request, Duration::from_millis(200));
    assert_eq!(result, SyncGameResult::Offline);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    assert!(b.cloud().wait_for_no_job(Duration::from_secs(10)));
    assert!(!b.save_dir(&tb).exists(), "nothing pulled after the budget");
}

#[test]
fn play_during_a_slow_pull_cancels_it_and_nothing_is_written_after_the_start() {
    let fake = Arc::new(FakeCloudProvider::new());
    let account = fake_account(&fake);
    let (a, b) = (TestHost::new("PC-A"), TestHost::new("PC-B"));
    let (ta, tb) = (a.game(SYSTEM, SHA1_A), b.game(SYSTEM, SHA1_A));
    a.write(&ta, "save.srm", b"v1");
    a.write(&ta, "rtc.bin", b"r1");
    a.sync_both(&account, &ta);
    let seen = slow_blobs(&fake, FakeOp::Read, Duration::from_millis(200));
    let worker = {
        let (b, tb, account) = (b.clone(), tb.clone(), account.clone());
        std::thread::spawn(move || b.pre_play(&account, &tb, SyncOptions::pull_only()))
    };
    seen.recv().unwrap();
    let started = Instant::now();
    let session = b.0.session.lock().unwrap();
    let guard = clear_units_for_play(b.cloud(), &session_unit_keys(&tb));
    assert_eq!(guard, PlayGuard::CancelledTransfer);
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "Play never waits on a transfer"
    );
    let runner_start = SystemTime::now();
    drop(session);
    assert_eq!(worker.join().unwrap(), SyncGameResult::Offline);
    let dir = b.save_dir(&tb);
    let written: Vec<_> = std::fs::read_dir(&dir).map_or_else(
        |_| Vec::new(),
        |entries| {
            entries
                .map(|e| e.unwrap().metadata().unwrap().modified().unwrap())
                .filter(|modified| *modified >= runner_start)
                .collect()
        },
    );
    assert!(
        written.is_empty(),
        "no unit file written after the runner start"
    );
}

#[test]
fn play_during_a_slow_push_cancels_it_and_the_push_never_commits() {
    let fake = Arc::new(FakeCloudProvider::new());
    let account = fake_account(&fake);
    let b = TestHost::new("PC-B");
    let tb = b.game(SYSTEM, SHA1_A);
    let unit = b.units(&tb)[0].clone();
    b.write(&tb, "save.srm", b"v1");
    b.write(&tb, "rtc.bin", b"r1");
    let seen = slow_blobs(&fake, FakeOp::Write, Duration::from_millis(200));
    let worker = {
        let (b, tb, account) = (b.clone(), tb.clone(), account.clone());
        std::thread::spawn(move || b.sync_both(&account, &tb))
    };
    seen.recv().unwrap();
    let guard = clear_units_for_play(b.cloud(), &session_unit_keys(&tb));
    assert_eq!(guard, PlayGuard::CancelledTransfer);
    assert_eq!(worker.join().unwrap(), SyncGameResult::Offline);
    assert_eq!(generation(&fake, &unit), 0, "cancelled push: no manifest");
}

#[test]
fn pre_play_pull_pre_empts_a_slow_background_push_of_another_game() {
    let fake = Arc::new(FakeCloudProvider::new());
    let account = fake_account(&fake);
    let (a, b) = (TestHost::new("PC-A"), TestHost::new("PC-B"));
    let (ta, tb_play) = (a.game(SYSTEM, SHA1_A), b.game(SYSTEM, SHA1_A));
    a.write(&ta, "save.srm", b"cloud");
    a.sync_both(&account, &ta);
    let tb_push = b.game(SYSTEM, SHA1_B);
    let push_unit = b.units(&tb_push)[0].clone();
    for slot in 1..=6 {
        let name = format!("save{slot}.srm");
        b.write(&tb_push, &name, name.as_bytes());
    }
    // ~3 s of uploads: waiting it out would blow the assertion below.
    let seen = slow_blobs(&fake, FakeOp::Write, Duration::from_millis(500));
    let pusher = {
        let (b, tb_push, account) = (b.clone(), tb_push.clone(), account.clone());
        std::thread::spawn(move || b.sync_both(&account, &tb_push))
    };
    seen.recv().unwrap();
    let started = Instant::now();
    let result = b.pre_play(&account, &tb_play, SyncOptions::pull_only());
    assert_eq!(result, SyncGameResult::Synced);
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "pull did not wait out the push: {:?}",
        started.elapsed()
    );
    assert_eq!(b.read(&tb_play, "save.srm").unwrap(), b"cloud");
    // F9: the pre-empted push is not dropped; it retries after the pull and commits.
    fake.set_hook(None);
    assert_eq!(pusher.join().unwrap(), SyncGameResult::Synced);
    assert_eq!(generation(&fake, &push_unit), 1, "retried push committed");
    assert_eq!(b.read(&tb_push, "save1.srm").unwrap(), b"save1.srm");
}

#[test]
fn play_waits_for_a_running_apply_pass_and_close_wait_too() {
    let b = TestHost::new("PC-B");
    let tb = b.game(SYSTEM, SHA1_A);
    let keys = session_unit_keys(&tb);
    let job = b
        .cloud()
        .claim(JobKind::Pass, Instant::now(), stop())
        .unwrap();
    let progress = job.enter_unit(&b.0.session, keys.first().unwrap()).unwrap();
    progress.set_phase_for_tests(SyncPhase::Applying);
    let releaser = {
        let progress = progress.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            progress.set_phase_for_tests(SyncPhase::Transferring);
        })
    };
    let started = Instant::now();
    assert_eq!(
        clear_units_for_play(b.cloud(), &keys),
        PlayGuard::WaitedForApply
    );
    assert!(started.elapsed() >= Duration::from_millis(140));
    releaser.join().unwrap();
    assert!(progress.is_cancelled(), "no new apply pass after Play");
    // Close-wait: the timeout cancels, then a running apply pass is still waited for.
    let fresh = job.enter_unit(&b.0.session, keys.first().unwrap()).unwrap();
    fresh.set_phase_for_tests(SyncPhase::Applying);
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        fresh.set_phase_for_tests(SyncPhase::Idle);
    });
    let started = Instant::now();
    let outcome = crate::cloud_close_wait::wait_for_idle(b.cloud(), Duration::from_millis(20));
    assert_eq!(outcome, crate::cloud_close_wait::WaitOutcome::Cancelled);
    assert!(started.elapsed() >= Duration::from_millis(140));
    releaser.join().unwrap();
}
