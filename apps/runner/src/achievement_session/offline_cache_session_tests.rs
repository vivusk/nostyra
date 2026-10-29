//! Offline starts over the answer cache: a game played online loads its cached set
//! offline (softcore, one "will sync" popup, unlocks journaled), unlocks delivered
//! earlier never pop again, and a game never played online gets exactly one popup
//! whether or not a login is cached.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use achievements::OFFLINE_NO_CACHE_BODY;
use ipc_protocol::RunnerEvent;

use super::achievement_session_test_support::{fake_with_set, nes_rom, BYTE_ACH};
use super::offline_cache_session_test_support::*;
use super::offline_session_watch::OFFLINE_SYNC_NOTICE;

fn no_cache_notice() -> String {
    format!("RetroAchievements: {OFFLINE_NO_CACHE_BODY}")
}

fn loaded(unlocked: u32, hardcore: bool) -> Option<RunnerEvent> {
    Some(RunnerEvent::AchievementsLoaded {
        ra_game_id: 1234,
        unlocked,
        total: 2,
        hardcore,
    })
}

#[test]
fn offline_run_loads_the_cached_set_and_journals_the_unlock() {
    let data = tempfile::tempdir().unwrap();
    let rom = nes_rom(data.path());
    let mut online = cached_run(fake_with_set(), data, &rom, false);
    online.frame();
    assert!(online.loaded());
    let mut run = cached_run(offline_fake(), finish(online), &rom, false);
    run.frame();
    assert_eq!(loaded_event(&run), loaded(0, false));
    assert_eq!(
        run.fake.kinds(),
        ["login2"],
        "sticky: set + session from the cache"
    );
    run.ram[0x10].set(5);
    run.frame();
    assert_eq!(
        run.unlocked(),
        [BYTE_ACH],
        "unlocks on the exact frame offline"
    );
    let journal = run.journal().list();
    assert_eq!(journal.len(), 1, "offline unlock journaled");
    assert_eq!(journal[0].1.id, BYTE_ACH);
    let notices = run.session.take_notices();
    let sync = notices.iter().filter(|n| *n == OFFLINE_SYNC_NOTICE).count();
    assert_eq!(sync, 1, "{notices:?}");
    assert!(!notices.contains(&no_cache_notice()), "{notices:?}");
    run.session.shutdown();
}

#[test]
fn unlocks_delivered_or_journaled_earlier_do_not_pop_again_offline() {
    for award_status in [200, -2] {
        let data = tempfile::tempdir().unwrap();
        let rom = nes_rom(data.path());
        let fake = fake_with_set();
        if award_status == -2 {
            fake.respond("awardachievement", -2, "offline");
        }
        let mut online = cached_run(fake, data, &rom, false);
        online.frame();
        online.ram[0x10].set(5);
        online.frame();
        assert_eq!(online.unlocked(), [BYTE_ACH]);
        let mut run = cached_run(offline_fake(), finish(online), &rom, false);
        run.frame();
        assert_eq!(loaded_event(&run), loaded(1, false), "award {award_status}");
        run.ram[0x10].set(5);
        run.frames(2);
        assert!(run.unlocked().is_empty(), "award {award_status}");
        let pending = usize::from(award_status == -2);
        assert_eq!(run.journal().list().len(), pending, "award {award_status}");
        run.session.shutdown();
    }
}

#[test]
fn cached_session_with_the_server_up_drops_hardcore_with_the_sync_notice() {
    let data = tempfile::tempdir().unwrap();
    let rom = nes_rom(data.path());
    let mut online = cached_run(fake_with_set(), data, &rom, false);
    online.frame();
    let fake = fake_with_set();
    fake.respond("startsession", -2, "offline");
    let mut run = cached_run(fake, finish(online), &rom, true);
    assert!(
        run.session.hardcore_gate().active(),
        "login answered online"
    );
    run.frame();
    assert_eq!(loaded_event(&run), loaded(0, false), "loads as softcore");
    assert!(!run.session.hardcore_gate().active());
    let notices = run.session.take_notices();
    assert!(
        notices.contains(&"Hardcore disabled: offline".to_owned()),
        "{notices:?}"
    );
    assert!(
        notices.contains(&OFFLINE_SYNC_NOTICE.to_owned()),
        "{notices:?}"
    );
    run.session.shutdown();
}

#[test]
fn never_played_game_offline_gets_exactly_one_notice_with_a_cached_login() {
    let data = tempfile::tempdir().unwrap();
    let other = other_rom(data.path());
    let mut online = cached_run(fake_with_set(), data, &other, false);
    online.frame();
    assert!(online.loaded(), "another game played online: login cached");
    let data = finish(online);
    let rom = nes_rom(data.path());
    let mut run = cached_run(offline_fake(), data, &rom, false);
    run.frames(2);
    assert!(!run.loaded());
    assert_eq!(run.session.take_notices(), [no_cache_notice()]);
    run.session.shutdown();
}

#[test]
fn offline_without_a_cached_login_gets_exactly_one_notice() {
    let data = tempfile::tempdir().unwrap();
    let rom = nes_rom(data.path());
    let mut run = cached_run(offline_fake(), data, &rom, false);
    run.frames(2);
    assert!(!run.loaded());
    assert_eq!(
        run.session.take_notices(),
        [no_cache_notice()],
        "no second \"Login required\" popup"
    );
    run.session.shutdown();
}
