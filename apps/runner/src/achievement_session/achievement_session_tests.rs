//! `AchievementSession` lifecycle over the fake server: set load after frame 1,
//! exact-frame unlock, offline journal, shutdown with a blocked request, token rejected.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::time::{Duration, Instant};

use achievements::{FakeReply, PendingJournal, RequestDispatch};
use ipc_protocol::RunnerEvent;

use super::achievement_event_routing::LOGIN_AGAIN;
use super::achievement_session_test_support::*;
use super::progress_of;

#[test]
fn inline_set_loads_in_frame_one_and_unlocks_on_the_exact_frame() {
    let mut h = harness(fake_with_set(), RequestDispatch::Inline);
    assert!(!h.loaded(), "no set request before memory regions exist");
    assert_eq!(h.fake.kinds(), ["login2"]);
    h.frame();
    let first = h.events.borrow()[0].clone();
    assert_eq!(
        first,
        RunnerEvent::AchievementsLoaded {
            ra_game_id: 1234,
            unlocked: 0,
            total: 2,
            hardcore: false
        }
    );
    h.frames(2);
    assert!(h.unlocked().is_empty());
    h.ram[0x10].set(5);
    h.frame();
    assert_eq!(
        h.unlocked(),
        [BYTE_ACH],
        "unlocks on the frame the byte changed"
    );
    assert!(h.fake.kinds().contains(&"awardachievement".to_owned()));
    assert!(
        h.journal().list().is_empty(),
        "delivered → journal entry removed"
    );
    let snapshot = achievements::load_game_snapshot(h.data.path(), "ABCDEF")
        .unwrap()
        .unwrap();
    assert_eq!(
        (snapshot.ra_game_id, snapshot.title.as_str()),
        (1234, "Test Game")
    );
    assert!(snapshot
        .achievements
        .iter()
        .any(|a| a.id == BYTE_ACH && a.unlocked));
    let notices = h.session.take_notices();
    assert!(
        notices
            .iter()
            .any(|n| n == "Achievement unlocked · 5 points: Ach 101"),
        "{notices:?}"
    );
    h.session.shutdown();
}

#[test]
fn offline_unlock_stays_in_the_journal_through_shutdown() {
    let fake = fake_with_set();
    fake.respond("awardachievement", -2, "offline");
    let mut h = harness(fake, RequestDispatch::Inline);
    h.frame();
    h.ram[0x10].set(5);
    h.frames(2);
    let entries = h.journal().list();
    assert_eq!(entries.len(), 1, "retries reuse one entry");
    let entry = &entries[0].1;
    assert_eq!(
        (entry.id, entry.user.as_str(), entry.hardcore),
        (BYTE_ACH, "Tester", false)
    );
    let Harness { session, data, .. } = h;
    session.shutdown();
    assert_eq!(
        PendingJournal::new(data.path()).list().len(),
        1,
        "kept for replay"
    );
}

#[test]
fn blocked_award_at_quit_does_not_hang_and_keeps_the_journal() {
    let fake = fake_with_set();
    fake.set_reply("awardachievement", FakeReply::Block);
    let mut h = harness(fake.clone(), RequestDispatch::Worker);
    h.wait_loaded();
    h.frame();
    h.ram[0x10].set(5);
    h.frame();
    assert_eq!(h.journal().list().len(), 1, "written when sent");
    let Harness { session, data, .. } = h;
    let started = Instant::now();
    session.shutdown();
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "worker detached"
    );
    assert_eq!(PendingJournal::new(data.path()).list().len(), 1);
    fake.release();
}

#[test]
fn rejected_token_stops_the_session_and_tells_the_launcher() {
    let fake = fake_with_set();
    let rejected = r#"{"Success":false,"Error":"Invalid token","Code":"invalid_credentials"}"#;
    fake.respond("login2", 401, rejected);
    let mut h = harness(fake, RequestDispatch::Inline);
    let events = h.events.borrow().clone();
    assert_eq!(events, [RunnerEvent::AchievementsLoginExpired]);
    assert!(h.session.take_notices().contains(&LOGIN_AGAIN.to_owned()));
    h.frames(2); // inert: no request, no panic
    assert_eq!(progress_of(&Some(h.session)), None);
}
