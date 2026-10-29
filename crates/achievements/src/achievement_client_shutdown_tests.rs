//! Exactly-once callbacks across timeouts and shutdown (D4/D5): every request issued
//! gets one callback on the client thread before `rc_client_destroy`, a blocked worker
//! cannot hang shutdown, and its late result is ignored.

#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use std::time::{Duration, Instant};

use super::test_support::{login_and_load, session, wait_for, ACH_BYTE};
use super::*;
use crate::fake_server_transport::FakeReply;
use crate::{ClientOp, RC_NO_RESPONSE};

#[test]
fn transport_timeout_reaches_the_caller_as_no_response_with_text() {
    let s = session(RequestDispatch::Worker, false);
    s.fake.respond("login2", -2, "timeout");
    s.client.begin_login_with_token("Tester", "tok123");
    let login = wait_for(&s.client, ClientOp::LoginWithToken);
    assert_eq!(
        (login.result, login.message.as_str()),
        (RC_NO_RESPONSE, "timeout")
    );
    let report = s.client.shutdown();
    assert_eq!(report.requests_issued, report.callbacks_fired);
    assert!(!report.worker_detached);
}

#[test]
fn login_in_flight_at_shutdown_fails_once_with_shutdown_text() {
    let s = session(RequestDispatch::Worker, false);
    s.fake.set_reply("login2", FakeReply::Block);
    s.client.begin_login_with_token("Tester", "tok123");
    assert_eq!(s.client.pending_requests(), 1);
    let fake = s.fake.clone();
    let started = Instant::now();
    let report = s.client.shutdown();
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
    assert_eq!((report.requests_issued, report.callbacks_fired), (1, 1));
    assert_eq!(report.failed_at_shutdown, 1);
    assert!(
        report.worker_detached,
        "worker blocked inside the login call"
    );
    assert_eq!(
        report.undrained_completions,
        [ClientCompletion {
            op: ClientOp::LoginWithToken,
            result: RC_NO_RESPONSE,
            message: "shutdown".into()
        }]
    );
    fake.release(); // the late login answer has no receiver left: dropped
}

#[test]
fn award_blocked_forever_still_shuts_down_with_every_callback_fired() {
    let s = session(RequestDispatch::Worker, true);
    login_and_load(&s);
    s.fake.set_reply("awardachievement", FakeReply::Block);
    for _ in 0..3 {
        s.client.do_frame();
    }
    s.ram.borrow_mut()[0x10] = 5;
    s.client.do_frame();
    assert!(matches!(
        &s.client.drain_events()[..],
        [AchievementEvent::AchievementTriggered(a)] if a.id == ACH_BYTE
    ));
    assert_eq!(
        s.client.pending_requests(),
        1,
        "award in flight on the worker"
    );
    let fake = s.fake.clone();
    let report = s.client.shutdown();
    // first −2 re-sends the award immediately (second request), second −2 schedules a
    // retry that destroy discards (journal is the durable record)
    assert_eq!(report.failed_at_shutdown, 2);
    assert_eq!(report.requests_issued, report.callbacks_fired);
    assert!(report.worker_detached);
    fake.release();
    let awards = fake
        .kinds()
        .iter()
        .filter(|k| *k == "awardachievement")
        .count();
    assert!(awards >= 1);
}

#[test]
fn dropping_the_client_runs_the_same_shutdown() {
    let s = session(RequestDispatch::Inline, true);
    login_and_load(&s);
    assert!(s.client.is_game_loaded());
    drop(s.client); // unload + destroy without leaking or firing after destroy
}
