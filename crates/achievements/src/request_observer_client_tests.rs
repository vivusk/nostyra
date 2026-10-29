//! `AchievementClient` award / submit observations (journal bookkeeping, QA L1) and
//! transport jobs, over the fake server.

#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::test_support::*;
use crate::{PendingKind, ReplayOutcome, RequestDispatch, RequestObservation};

fn unlock_byte_achievement(s: &Session) -> Vec<RequestObservation> {
    login_and_load(s);
    s.client.do_frame();
    s.ram.borrow_mut()[0x10] = 5;
    s.client.do_frame();
    s.client.drain_request_observations()
}

#[test]
fn delivered_award_is_observed_sent_then_answered() {
    let s = session(RequestDispatch::Inline, true);
    let log = unlock_byte_achievement(&s);
    let [RequestObservation::Sent(sent), RequestObservation::Answered(answered, outcome)] =
        &log[..]
    else {
        panic!("{log:?}");
    };
    assert_eq!(
        (sent.kind, sent.id, sent.hardcore),
        (PendingKind::Unlock, ACH_BYTE, false)
    );
    assert_eq!(sent.game_hash, HASH);
    assert_eq!((answered, outcome), (sent, &ReplayOutcome::Delivered));
}

#[test]
fn offline_award_is_answered_retry_and_resent() {
    let s = session(RequestDispatch::Inline, true);
    s.fake.respond("awardachievement", -2, "offline");
    let log = unlock_byte_achievement(&s);
    assert!(
        matches!(
            &log[1],
            RequestObservation::Answered(_, ReplayOutcome::Retry(_))
        ),
        "{log:?}"
    );
    assert!(
        matches!(&log[2], RequestObservation::Sent(t) if t.id == ACH_BYTE),
        "{log:?}"
    );
}

#[test]
fn transport_jobs_run_inline_now_and_on_the_worker_later() {
    let s = session(RequestDispatch::Inline, false);
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    s.client
        .run_transport_job(Box::new(move |_| flag.store(true, Ordering::SeqCst)));
    assert!(ran.load(Ordering::SeqCst));

    let s = session(RequestDispatch::Worker, false);
    let (tx, rx) = std::sync::mpsc::channel();
    s.client.run_transport_job(Box::new(move |transport| {
        let request = crate::ServerRequest {
            url: "https://x/dorequest.php".into(),
            post_data: "r=ping".into(),
            content_type: String::new(),
        };
        tx.send(transport.call(&request).status).unwrap();
    }));
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(),
        200
    );
    assert_eq!(s.fake.kinds(), ["ping"]);
}
