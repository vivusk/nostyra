#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::fake_server_transport::{FakeReply, FakeTransport};
use crate::ffi::test_callbacks::{recording, FIRED};
use crate::server_transport::BODY_SHUTDOWN;

fn request(kind: &str) -> ServerRequest {
    ServerRequest {
        url: "https://retroachievements.org/dorequest.php".into(),
        post_data: format!("r={kind}&u=Tester"),
        content_type: "application/x-www-form-urlencoded".into(),
    }
}

fn fired() -> Vec<(usize, i32, String)> {
    FIRED.with(|fired| fired.borrow().clone())
}

fn clear_fired() {
    FIRED.with(|fired| fired.borrow_mut().clear());
}

/// Polls `complete_ready` until `want` callbacks fired or 5 s passed.
fn pump_until(queue: &RequestQueue, want: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut total = 0;
    while total < want && Instant::now() < deadline {
        total += queue.complete_ready();
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn inline_dispatch_fires_inside_submit() {
    clear_fired();
    let fake = Arc::new(FakeTransport::new());
    fake.respond("ping", 200, "{\"Success\":true}");
    let queue = RequestQueue::new(fake.clone(), RequestDispatch::Inline);
    queue.submit(request("ping"), recording(7));
    assert_eq!(fired(), [(7, 200, "{\"Success\":true}".to_owned())]);
    assert_eq!(queue.pending_count(), 0);
    assert_eq!(queue.counters(), (1, 1));
    assert_eq!(fake.kinds(), ["ping"]);
}

#[test]
fn worker_results_fire_only_when_drained_and_exactly_once() {
    clear_fired();
    let fake = Arc::new(FakeTransport::new());
    let queue = RequestQueue::new(fake, RequestDispatch::Worker);
    queue.submit(request("ping"), recording(1));
    queue.submit(request("startsession"), recording(2));
    assert!(
        fired().is_empty(),
        "nothing fires before the client thread drains"
    );
    pump_until(&queue, 2);
    let tags: Vec<usize> = fired().iter().map(|f| f.0).collect();
    assert_eq!(tags, [1, 2], "FIFO order");
    // late / duplicate results for known-done or unknown ids are dropped
    assert!(!queue.complete(1, &ServerResponse::no_response("timeout")));
    assert!(!queue.complete(99, &ServerResponse::no_response("timeout")));
    assert_eq!(fired().len(), 2);
    assert_eq!(queue.counters(), (2, 2));
}

#[test]
fn fail_all_pending_fires_minus_two_with_reason() {
    clear_fired();
    let fake = Arc::new(FakeTransport::new());
    fake.set_reply("awardachievement", FakeReply::Block);
    let mut queue = RequestQueue::new(fake.clone(), RequestDispatch::Worker);
    queue.submit(request("awardachievement"), recording(3));
    queue.submit(request("ping"), recording(4));
    assert_eq!(queue.fail_all_pending(BODY_SHUTDOWN), 2);
    let all = fired();
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|f| f.1 == -2 && f.2 == "shutdown"));
    assert_eq!(queue.pending_count(), 0);

    // the worker is stuck inside the blocked award: shutdown detaches it in ~2 s
    let started = Instant::now();
    assert!(queue.close_worker(Duration::from_millis(300)));
    assert!(started.elapsed() < Duration::from_secs(2));
    fake.release();
    // its late result has nowhere to go; nothing fires twice
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(queue.complete_ready(), 0);
    assert_eq!(fired().len(), 2);
}

#[test]
fn worker_that_finishes_is_joined_not_detached() {
    let mut queue = RequestQueue::new(Arc::new(FakeTransport::new()), RequestDispatch::Worker);
    assert!(!queue.close_worker(WORKER_JOIN_TIMEOUT));
    // closed queue: further submits complete immediately as offline
    clear_fired();
    queue.submit(request("ping"), recording(5));
    assert_eq!(fired(), [(5, -2, "offline".to_owned())]);
}

#[test]
fn background_jobs_wait_for_queued_requests_and_jobs() {
    clear_fired();
    let fake = Arc::new(FakeTransport::new());
    let queue = RequestQueue::new(fake.clone(), RequestDispatch::Worker);
    let order = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let (release, gate) = std::sync::mpsc::channel::<()>();
    // A normal job holds the worker while the rest is queued behind it.
    queue.run_job(
        Box::new(move |_| {
            let _ = gate.recv();
        }),
        false,
    );
    let (bg_order, bg_fake) = (order.clone(), fake.clone());
    queue.run_job(
        Box::new(move |_| {
            let saw_ping = bg_fake.kinds().contains(&"ping".to_owned());
            bg_order
                .lock()
                .unwrap()
                .push(format!("bg ping_done={saw_ping}"));
        }),
        true,
    );
    queue.submit(request("ping"), recording(3));
    let job_order = order.clone();
    queue.run_job(
        Box::new(move |_| job_order.lock().unwrap().push("job".into())),
        false,
    );
    release.send(()).unwrap();
    pump_until(&queue, 1);
    let deadline = Instant::now() + Duration::from_secs(5);
    while order.lock().unwrap().len() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(*order.lock().unwrap(), ["job", "bg ping_done=true"]);
    assert_eq!(fired().len(), 1);
}
