#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use super::test_support::*;
use super::*;
use crate::{ClientOp, RC_NO_GAME_LOADED};

fn frames(s: &Session, n: usize) {
    for _ in 0..n {
        s.client.do_frame();
    }
}

#[test]
fn token_login_reports_user() {
    let s = session(RequestDispatch::Inline, false);
    s.client.begin_login_with_token("Tester", "tok123");
    assert_eq!(
        s.client.drain_completions(),
        [ClientCompletion {
            op: ClientOp::LoginWithToken,
            result: 0,
            message: String::new()
        }]
    );
    let user = s.client.user_info().unwrap();
    assert_eq!((user.username.as_str(), user.score), ("Tester", 10));
    assert_eq!(s.fake.kinds(), ["login2"]);
}

#[test]
fn unknown_game_fails_with_no_game_loaded_and_keeps_the_hash() {
    let s = session(RequestDispatch::Inline, false);
    s.client.begin_login_with_token("Tester", "tok123");
    s.client.begin_load_game(HASH);
    let load = wait_for(&s.client, ClientOp::LoadGame);
    assert_eq!(load.result, RC_NO_GAME_LOADED);
    let game = s.client.game_info().unwrap();
    assert_eq!((game.id, game.hash.as_str()), (0, HASH));
}

#[test]
fn identify_hashes_the_given_buffer() {
    let s = session(RequestDispatch::Inline, false);
    s.client.begin_login_with_token("Tester", "tok123");
    let rom: Vec<u8> = (0..32 * 1024u32).map(|i| (i % 251) as u8).collect();
    s.client
        .begin_identify_and_load_game(7, Path::new("game.nes"), Some(rom));
    assert_eq!(
        wait_for(&s.client, ClientOp::LoadGame).result,
        RC_NO_GAME_LOADED
    );
    let hash = s.client.game_info().unwrap().hash;
    assert_eq!(hash.len(), 32, "md5 hex: {hash}");
    let sent = s
        .fake
        .posts()
        .into_iter()
        .find(|p| p.starts_with("r=achievementsets"));
    assert!(sent.unwrap().contains(&format!("m={hash}")));
}

#[test]
fn achievement_triggers_on_the_exact_frame_and_is_awarded() {
    let s = session(RequestDispatch::Inline, true);
    login_and_load(&s);
    assert_eq!(s.client.load_game_state(), LoadGameState::Done);
    frames(&s, 5);
    s.ram.borrow_mut()[0x10] = 5;
    s.client.do_frame();
    let events = s.client.drain_events();
    assert!(
        matches!(&events[..], [AchievementEvent::AchievementTriggered(a)] if a.id == ACH_BYTE && a.title == "Ach 101" && a.badge_name == "00101"),
        "{events:?}"
    );
    assert_eq!(
        s.fake.kinds(),
        [
            "login2",
            "achievementsets",
            "startsession",
            "awardachievement"
        ]
    );
    let summary = s.client.user_game_summary();
    assert_eq!(
        (
            summary.num_core_achievements,
            summary.num_unlocked_achievements
        ),
        (2, 1)
    );
    let listed: usize = s
        .client
        .achievement_list(1, 0)
        .iter()
        .map(|b| b.achievements.len())
        .sum();
    assert_eq!(listed, 2);
    assert_eq!(s.client.game_info().unwrap().title, "Test Game");
}

#[test]
fn hardcore_leaderboard_reports_tracker_and_submit() {
    let s = session(RequestDispatch::Inline, true);
    s.client.set_hardcore_enabled(true);
    assert!(s.client.hardcore_enabled());
    login_and_load(&s);
    frames(&s, 2);
    s.ram.borrow_mut()[0x22] = 17;
    s.ram.borrow_mut()[0x20] = 1;
    s.client.do_frame(); // STA
    s.ram.borrow_mut()[0x20] = 2;
    s.client.do_frame(); // SUB
    let events = s.client.drain_events();
    assert!(events
        .iter()
        .any(|e| matches!(e, AchievementEvent::LeaderboardStarted(l) if l.id == LBOARD)));
    assert!(
        events.iter().any(
            |e| matches!(e, AchievementEvent::TrackerShow { display, .. } if display == "000017")
        ),
        "{events:?}"
    );
    assert!(events
        .iter()
        .any(|e| matches!(e, AchievementEvent::LeaderboardSubmitted(l) if l.id == LBOARD)));
    assert!(s.fake.kinds().contains(&"submitlbentry".to_owned()));
    assert_eq!(
        s.client
            .leaderboard_list(0)
            .iter()
            .map(|b| b.leaderboards.len())
            .sum::<usize>(),
        1
    );
}

#[test]
fn progress_round_trip_and_reset() {
    let s = session(RequestDispatch::Inline, true);
    assert_eq!(s.client.serialize_progress(), Err(RC_NO_GAME_LOADED));
    login_and_load(&s);
    frames(&s, 3);
    s.ram.borrow_mut()[0x30] = 1;
    frames(&s, 5); // 5 of 10 hits
    let blob = s.client.serialize_progress().unwrap();
    frames(&s, 3);
    s.client.deserialize_progress(Some(&blob)).unwrap();
    frames(&s, 4);
    assert!(
        s.client.drain_events().is_empty(),
        "restored to 5 hits: 9 after 4 frames"
    );
    s.client.do_frame();
    assert_eq!(s.client.drain_events().len(), 1, "10th hit triggers");
    s.client.deserialize_progress(None).unwrap();
    s.client.reset();
    s.client.idle();
    assert!(s.client.can_pause().0, "softcore never limits pause");
}
