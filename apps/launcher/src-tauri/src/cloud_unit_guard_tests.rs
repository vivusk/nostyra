//! Unit guards beyond Play (F4, F5): the shared PS2 memory-card unit is skipped while
//! any PS2 game runs (and a Play of another PS2 game cancels a job on it); folder
//! writers are `busy` while a cloud job or the game holds the unit.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;

use cloud_sync::providers::fake_cloud_provider::FakeCloudProvider;
use cloud_sync::SyncPhase;

use crate::cloud_game_result::SyncGameResult;
use crate::cloud_sync_state::JobKind;
use crate::cloud_test_harness::{
    fake_account, generation, ps2_system, TestHost, SHA1_A, SHA1_B, SYSTEM,
};
use crate::cloud_unit_guard::{
    clear_units_for_play, ensure_unit_idle, session_unit_keys, PlayGuard,
};

fn stop() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

#[test]
fn shared_ps2_unit_is_skipped_while_another_ps2_game_runs() {
    let fake = Arc::new(FakeCloudProvider::new());
    let account = fake_account(&fake);
    let pc = TestHost::new("PC-A");
    let (game_a, game_b) = (pc.game(ps2_system(), SHA1_A), pc.game(ps2_system(), SHA1_B));
    let units = pc.units(&game_a);
    assert_eq!(units.len(), 2, "game unit + shared memory cards");
    pc.write(&game_a, "slot1.state", b"s");
    let memcards = units[1].local_dir().to_path_buf();
    std::fs::create_dir_all(&memcards).unwrap();
    std::fs::write(memcards.join("Mcd001.ps2"), b"card").unwrap();
    pc.start_session(&game_b);
    assert_eq!(pc.sync_both(&account, &game_a), SyncGameResult::Synced);
    assert_eq!(generation(&fake, &units[0]), 1, "game A unit synced");
    assert_eq!(
        generation(&fake, &units[1]),
        0,
        "shared unit in use: skipped"
    );
    // Play of PS2 game B while the shared unit is in a job: the transfer is cancelled.
    *pc.0.session.lock().unwrap() = None;
    let job = pc
        .cloud()
        .claim(JobKind::Pass, Instant::now(), stop())
        .unwrap();
    let progress = job.enter_unit(&pc.0.session, &units[1].key()).unwrap();
    progress.set_phase_for_tests(SyncPhase::Transferring);
    let guard = clear_units_for_play(pc.cloud(), &session_unit_keys(&game_b));
    assert_eq!(guard, PlayGuard::CancelledTransfer);
    assert!(progress.is_cancelled());
}

#[test]
fn folder_writers_are_busy_while_a_job_holds_the_unit() {
    let pc = TestHost::new("PC-A");
    let game = pc.game(SYSTEM, SHA1_A);
    let keys = session_unit_keys(&game);
    assert!(ensure_unit_idle(None, pc.cloud(), 1, &game).is_ok());
    let job = pc
        .cloud()
        .claim(JobKind::Push, Instant::now(), stop())
        .unwrap();
    job.enter_unit(&pc.0.session, keys.first().unwrap())
        .unwrap();
    let err = ensure_unit_idle(None, pc.cloud(), 1, &game).unwrap_err();
    assert_eq!(err.kind, "busy");
    job.leave_unit();
    assert!(ensure_unit_idle(None, pc.cloud(), 1, &game).is_ok());
    drop(job);
    pc.start_session(&game);
    let session = pc.0.session.lock().unwrap();
    let running = session.as_ref().map(|s| crate::app_state::ActiveSession {
        game_id: 1,
        ..s.clone()
    });
    let err = ensure_unit_idle(running.as_ref(), pc.cloud(), 1, &game).unwrap_err();
    assert_eq!(err.kind, "busy");
}
