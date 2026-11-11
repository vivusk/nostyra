//! The job slot: one job at a time, pre-play pulls jump ahead of a background pass
//! at a unit boundary, units held by a running game are refused, stop is sticky.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;
use crate::app_state::ActiveSession;
use crate::cloud_job_guard::UnitRefused;

fn stop() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[test]
fn one_job_at_a_time() {
    let cloud = CloudSyncState::default();
    let first = cloud.claim(JobKind::Push, Instant::now(), stop()).unwrap();
    let started = Instant::now();
    let soon = Instant::now() + Duration::from_millis(50);
    assert!(cloud.claim(JobKind::Pass, soon, stop()).is_none());
    assert!(started.elapsed() >= Duration::from_millis(40));
    assert!(cloud.job_running());
    drop(first);
    assert!(!cloud.job_running());
    assert!(cloud.claim(JobKind::Pass, Instant::now(), stop()).is_some());
}

#[test]
fn a_waiting_pre_play_pull_gets_the_slot_between_units_of_a_pass() {
    let cloud = Arc::new(CloudSyncState::default());
    let pass = cloud.claim(JobKind::Pass, Instant::now(), stop()).unwrap();
    let (claimed, got) = std::sync::mpsc::channel();
    let player = {
        let cloud = cloud.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let guard = cloud.claim(JobKind::PrePlay, deadline, stop());
            claimed.send(guard.is_some()).unwrap();
            std::thread::sleep(Duration::from_millis(50));
        })
    };
    while cloud.play_waiters.load(Ordering::SeqCst) == 0 {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        pass.yield_to_play(),
        "the pass takes the slot back afterwards"
    );
    assert!(got.recv().unwrap(), "pre-play pull ran in between");
    player.join().unwrap();
    assert!(cloud.job_running(), "the pass holds the slot again");
    drop(pass);
    assert!(!cloud.job_running());
}

#[test]
fn a_pre_play_claim_cancels_the_pass_unit_but_does_not_stop_the_pass() {
    let cloud = Arc::new(CloudSyncState::default());
    let session = Mutex::new(None);
    let pass_stop = stop();
    let pass = cloud
        .claim(JobKind::Pass, Instant::now(), pass_stop.clone())
        .unwrap();
    let progress = pass.enter_unit(&session, "sys/aaaa").unwrap();
    let player = {
        let cloud = cloud.clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            cloud.claim(JobKind::PrePlay, deadline, stop()).is_some()
        })
    };
    while !progress.is_cancelled() {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        !pass_stop.load(Ordering::SeqCst),
        "the pass goes on afterwards"
    );
    pass.leave_unit();
    assert!(pass.yield_to_play());
    assert!(player.join().unwrap());
}

#[test]
fn units_of_a_running_game_and_stopped_jobs_are_refused() {
    let cloud = CloudSyncState::default();
    let session = Mutex::new(Some(ActiveSession {
        game_id: 1,
        unit_keys: BTreeSet::from(["sys/aaaa".to_string()]),
        control: None,
    }));
    let job_stop = stop();
    let job = cloud
        .claim(JobKind::Pass, Instant::now(), job_stop.clone())
        .unwrap();
    assert_eq!(
        job.enter_unit(&session, "sys/aaaa").unwrap_err(),
        UnitRefused::InUse
    );
    let progress = job.enter_unit(&session, "sys/bbbb").unwrap();
    let keys = BTreeSet::from(["sys/bbbb".to_string()]);
    assert!(cloud.job_progress_for(&keys).is_some());
    job.leave_unit();
    assert!(cloud.job_progress_for(&keys).is_none());
    assert!(cloud.stop_running().is_some());
    assert!(progress.is_cancelled() || job_stop.load(Ordering::SeqCst));
    assert_eq!(
        job.enter_unit(&session, "sys/bbbb").unwrap_err(),
        UnitRefused::Stopped
    );
    assert!(!job.yield_to_play());
}

#[test]
fn closing_refuses_new_jobs() {
    let cloud = CloudSyncState::default();
    cloud.closing.store(CLOSE_WAITING, Ordering::SeqCst);
    assert!(cloud.claim(JobKind::Push, Instant::now(), stop()).is_none());
}
