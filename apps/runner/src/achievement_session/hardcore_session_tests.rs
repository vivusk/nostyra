//! Hardcore in the session (flag forced on by passing `hardcore_ui`, as the debug
//! env `NOSTYRA_RA_HARDCORE_UI=1` does in the runner): hidden flag → softcore, pause
//! limited by `rc_client_can_pause`, disallowed core option and rc_client's own
//! disable lift the shared gate, a stopped client lifts it too.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use achievements::RequestDispatch;
use config_layers::AchievementsConfig;
use ipc_protocol::RunnerEvent;

use super::achievement_session_test_support::*;
use super::hardcore_session_gate::pause_wait_text;

fn hardcore_on() -> AchievementsConfig {
    AchievementsConfig {
        hardcore: true,
        ..AchievementsConfig::default()
    }
}

fn loaded_hardcore(h: &Harness) -> Option<bool> {
    h.events.borrow().iter().find_map(|e| match e {
        RunnerEvent::AchievementsLoaded { hardcore, .. } => Some(*hardcore),
        _ => None,
    })
}

#[test]
fn hidden_flag_keeps_the_session_softcore_whatever_the_setting() {
    let launch = HardcoreLaunch::default(); // flag off (release behaviour)
    let mut h = harness_with(
        fake_with_set(),
        RequestDispatch::Inline,
        &hardcore_on(),
        launch,
    );
    assert!(!h.session.hardcore_gate().active());
    h.frame();
    assert_eq!(loaded_hardcore(&h), Some(false));
    assert_eq!(
        h.session.pause_refusal(60.0),
        None,
        "softcore pauses freely"
    );
}

#[test]
fn hardcore_session_limits_pauses_with_rc_client() {
    let launch = HardcoreLaunch {
        hardcore_ui: true,
        ..HardcoreLaunch::default()
    };
    let mut h = harness_with(
        fake_with_set(),
        RequestDispatch::Inline,
        &hardcore_on(),
        launch,
    );
    let gate = h.session.hardcore_gate();
    assert!(gate.active());
    h.frame();
    assert_eq!(loaded_hardcore(&h), Some(true));
    assert_eq!(h.session.pause_refusal(60.0), None, "first pause allowed");
    h.frame(); // one unpaused frame only
    assert_eq!(
        h.session.pause_refusal(60.0).as_deref(),
        Some("Pause available in 1 s")
    );
    h.frames(80);
    assert_eq!(h.session.pause_refusal(60.0), None, "allowed again later");
    h.session.drop_to_softcore("test");
    assert!(!gate.active(), "every holder sees the drop");
    h.frame();
    assert_eq!(h.session.pause_refusal(60.0), None);
}

#[test]
fn disallowed_core_option_starts_the_session_in_softcore() {
    let options = vec![("mesen_region".to_owned(), "PAL".to_owned())];
    let launch = HardcoreLaunch {
        core_options: &options,
        hardcore_ui: true,
    };
    let mut h = harness_with(
        fake_with_set(),
        RequestDispatch::Inline,
        &hardcore_on(),
        launch,
    );
    assert!(!h.session.hardcore_gate().active());
    h.frame();
    assert_eq!(loaded_hardcore(&h), Some(false), "rc_client in softcore");
    let notices = h.session.take_notices();
    assert!(
        notices
            .iter()
            .any(|n| n == "Hardcore disabled: core option mesen_region=PAL not allowed"),
        "{notices:?}"
    );
}

#[test]
fn rc_client_disable_and_client_stop_lift_the_gate() {
    let launch = HardcoreLaunch {
        hardcore_ui: true,
        ..HardcoreLaunch::default()
    };
    let config = hardcore_on();
    let mut h = harness_with(fake_with_set(), RequestDispatch::Inline, &config, launch);
    let gate = h.session.hardcore_gate();
    h.frame();
    // What rc_client does itself on an unknown disc (RC_HARDCORE_DISABLED).
    h.session
        .client
        .as_ref()
        .unwrap()
        .set_hardcore_enabled(false);
    h.frame();
    assert!(!gate.active(), "gates follow rc_client");

    let launch = HardcoreLaunch {
        hardcore_ui: true,
        ..HardcoreLaunch::default()
    };
    let mut h = harness_with(fake_with_set(), RequestDispatch::Inline, &config, launch);
    let gate = h.session.hardcore_gate();
    h.session.stop(); // token rejected: achievements end for this session
    assert!(!gate.active());
}

#[test]
fn pause_wait_rounds_up_to_whole_seconds() {
    assert_eq!(pause_wait_text(19, 60.0), "Pause available in 1 s");
    assert_eq!(pause_wait_text(61, 60.0), "Pause available in 2 s");
    assert_eq!(pause_wait_text(0, 60.0), "Pause available in 1 s");
    assert_eq!(
        pause_wait_text(120, 0.0),
        "Pause available in 2 s",
        "bad fps"
    );
}
