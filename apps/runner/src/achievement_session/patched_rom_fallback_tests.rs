//! Patched ROM fallback: an unknown soft-patched game loads the original ROM's set
//! (softcore, one popup on success); a known patched hash, an unpatched game or the
//! setting off never retry; a second unknown answer shows the patched hash.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use achievements::RequestDispatch;
use config_layers::AchievementsConfig;
use ipc_protocol::RunnerEvent;

use super::achievement_session_test_support::{fake_with_set, harness, BYTE_ACH};
use super::patched_rom_fallback::PATCHED_FALLBACK_NOTICE;
use super::patched_rom_fallback_test_support::{hashes_of, patched_run, PatchedRun};

const UNKNOWN_PLACARD: &str = "RetroAchievements: Unknown game — achievements inactive";

fn loaded_softcore() -> RunnerEvent {
    RunnerEvent::AchievementsLoaded {
        ra_game_id: 1234,
        unlocked: 0,
        total: 2,
        hardcore: false,
    }
}

/// A fake that knows only the set of `hash` (others: unknown game).
fn fake_knowing(hash: &str) -> std::sync::Arc<achievements::FakeTransport> {
    let fake = fake_with_set();
    fake.answer_set_only_for(hash);
    fake
}

/// Starts a patched run; its fake then knows only the hash `pick` chooses (the set
/// request waits for the first frame, so this is in time).
fn run_with(
    config: &AchievementsConfig,
    hardcore: bool,
    pick: fn(&PatchedRun) -> String,
) -> PatchedRun {
    let patched = patched_run(fake_with_set(), config, hardcore);
    patched.run.fake.answer_set_only_for(&pick(&patched));
    patched
}

#[test]
fn unknown_patched_game_loads_the_original_set_softcore() {
    let config = AchievementsConfig::default();
    let PatchedRun {
        mut run,
        original_hash,
        patched_hash,
    } = run_with(&config, true, |p| p.original_hash.clone());
    run.frame();
    assert_eq!(run.events.borrow().first(), Some(&loaded_softcore()));
    assert_eq!(
        hashes_of(&run.fake, "achievementsets"),
        [patched_hash, original_hash.clone()]
    );
    assert_eq!(hashes_of(&run.fake, "startsession"), [original_hash]);
    assert!(!run.session.hardcore_gate().active(), "gates lifted");
    run.ram[0x10].set(5);
    run.frame();
    assert_eq!(run.unlocked(), [BYTE_ACH], "unlocks on the exact frame");
    assert!(run
        .fake
        .posts()
        .iter()
        .any(|p| p.contains("r=awardachievement") && p.contains("h=0")));
    let notices = run.session.take_notices();
    assert!(
        notices
            .iter()
            .any(|n| n == "Hardcore disabled: patched ROM"),
        "{notices:?}"
    );
    assert_eq!(
        notices
            .iter()
            .filter(|n| *n == PATCHED_FALLBACK_NOTICE)
            .count(),
        1,
        "{notices:?}"
    );
    assert!(
        notices.iter().any(|n| n.contains("Test Game")),
        "loaded placard: {notices:?}"
    );
    assert!(!notices.iter().any(|n| n == UNKNOWN_PLACARD), "{notices:?}");
    run.session.shutdown();
}

#[test]
fn second_unknown_answer_shows_the_patched_hash_without_the_fallback_popup() {
    let PatchedRun {
        mut run,
        original_hash,
        patched_hash,
    } = run_with(&AchievementsConfig::default(), false, |_| "0".repeat(32));
    run.frame();
    run.frame();
    assert_eq!(
        hashes_of(&run.fake, "achievementsets"),
        [patched_hash.clone(), original_hash.clone()],
        "one retry only"
    );
    assert!(!run.loaded());
    assert_eq!(
        run.session.router.unknown_placard_hash(original_hash),
        patched_hash
    );
    let notices = run.session.take_notices();
    assert_eq!(
        notices.iter().filter(|n| *n == UNKNOWN_PLACARD).count(),
        1,
        "{notices:?}"
    );
    assert!(
        !notices.iter().any(|n| n == PATCHED_FALLBACK_NOTICE),
        "{notices:?}"
    );
    run.session.shutdown();
}

#[test]
fn setting_off_shows_the_unknown_placard() {
    let config = AchievementsConfig {
        patched_rom_fallback: false,
        ..AchievementsConfig::default()
    };
    let PatchedRun {
        mut run,
        patched_hash,
        ..
    } = run_with(&config, false, |p| p.original_hash.clone());
    run.frame();
    assert_eq!(hashes_of(&run.fake, "achievementsets"), [patched_hash]);
    assert!(!run.loaded());
    let notices = run.session.take_notices();
    assert!(notices.iter().any(|n| n == UNKNOWN_PLACARD), "{notices:?}");
    run.session.shutdown();
}

#[test]
fn known_patched_hash_loads_directly() {
    let PatchedRun {
        mut run,
        patched_hash,
        ..
    } = run_with(&AchievementsConfig::default(), false, |p| {
        p.patched_hash.clone()
    });
    run.frame();
    assert_eq!(run.events.borrow().first(), Some(&loaded_softcore()));
    assert_eq!(hashes_of(&run.fake, "achievementsets"), [patched_hash]);
    let notices = run.session.take_notices();
    assert!(
        !notices.iter().any(|n| n == PATCHED_FALLBACK_NOTICE),
        "{notices:?}"
    );
    run.session.shutdown();
}

#[test]
fn unpatched_unknown_game_never_retries() {
    let fake = fake_knowing(&"0".repeat(32));
    let mut h = harness(fake, RequestDispatch::Inline);
    h.frames(2);
    assert_eq!(hashes_of(&h.fake, "achievementsets").len(), 1);
    assert!(h.session.router.patched_fallback.is_none());
    let notices = h.session.take_notices();
    assert!(notices.iter().any(|n| n == UNKNOWN_PLACARD), "{notices:?}");
    h.session.shutdown();
}
