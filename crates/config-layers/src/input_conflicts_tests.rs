//! Conflict rules: hotkey wins, first hotkey wins, devices apart, hold hotkeys
//! yield to the system's game actions, silent defaults, unused actions dropped.

use super::*;

/// PlayStation pad (with L2/R2) and SNES pad (without), as the registry lists them.
const PSX: &[&str] = &[
    "b", "a", "y", "x", "select", "start", "up", "down", "left", "right", "l", "r", "l2", "r2",
];
const SNES: &[&str] = &[
    "b", "y", "select", "start", "up", "down", "left", "right", "a", "x", "l", "r",
];
/// Nintendo 64: Z on `l2`, both sticks.
const N64: &[&str] = &[
    "b",
    "y",
    "l2",
    "l",
    "r",
    "start",
    "up",
    "down",
    "left",
    "right",
    "lstick_up",
    "lstick_down",
    "lstick_left",
    "lstick_right",
    "rstick_up",
    "rstick_down",
    "rstick_left",
    "rstick_right",
];

fn binds(input: &mut Binds, key: &str, values: &[&str]) {
    input.insert(
        key.into(),
        values.iter().map(|s| (*s).to_string()).collect(),
    );
}

#[test]
fn pristine_defaults_resolve_silently_in_every_view() {
    for system in [None, Some(SNES), Some(PSX), Some(N64)] {
        let mut input = InputConfig::default();
        assert!(
            resolve_input_conflicts(&mut input, system).is_empty(),
            "{system:?}"
        );
    }
}

#[test]
fn snes_keeps_trigger_hotkeys_and_drops_l2_r2_silently() {
    let mut input = InputConfig::default();
    assert!(resolve_input_conflicts(&mut input, Some(SNES)).is_empty());
    assert_eq!(input.hotkeys_gamepad["rewind_hold"], ["left_trigger"]);
    assert_eq!(
        input.hotkeys_gamepad["fast_forward_hold"],
        ["right_trigger"]
    );
    assert!(input.gamepad["l2"].is_empty() && input.gamepad["r2"].is_empty());
    assert!(
        input.keyboard["l2"].is_empty(),
        "unused actions never reach the core"
    );
    // No-game view: hotkeys win too (silently between defaults); no system, so the
    // second shoulder pair keeps its keyboard keys.
    let mut none = InputConfig::default();
    assert!(resolve_input_conflicts(&mut none, None).is_empty());
    assert_eq!(none.hotkeys_gamepad, input.hotkeys_gamepad);
    assert!(none.gamepad["l2"].is_empty());
    assert_eq!(none.keyboard["l2"], ["KeyE"]);
}

#[test]
fn n64_z_on_left_trigger_yields_rewind_silently_and_keeps_stick_keys() {
    let mut input = InputConfig::default();
    assert!(resolve_input_conflicts(&mut input, Some(N64)).is_empty());
    assert_eq!(input.gamepad["l2"], ["left_trigger"], "Z");
    assert!(input.hotkeys_gamepad["rewind_hold"].is_empty());
    assert_eq!(
        input.hotkeys_gamepad["fast_forward_hold"],
        ["right_trigger"],
        "N64 has no r2"
    );
    assert_eq!(input.keyboard["lstick_left"], ["KeyJ"]);
    assert_eq!(input.keyboard["rstick_up"], ["KeyT"]);
    // SNES: no stick actions → their keys never reach the core.
    let mut snes = InputConfig::default();
    assert!(resolve_input_conflicts(&mut snes, Some(SNES)).is_empty());
    assert!(snes.keyboard["lstick_left"].is_empty());
    assert_eq!(snes.hotkeys_gamepad["rewind_hold"], ["left_trigger"]);
}

#[test]
fn psx_triggers_become_l2_r2_and_warn_only_for_user_bindings() {
    let mut input = InputConfig::default();
    assert!(resolve_input_conflicts(&mut input, Some(PSX)).is_empty());
    assert_eq!(input.gamepad["l2"], ["left_trigger"]);
    assert_eq!(input.gamepad["r2"], ["right_trigger"]);
    assert!(input.hotkeys_gamepad["rewind_hold"].is_empty());
    assert!(input.hotkeys_gamepad["fast_forward_hold"].is_empty());
    // A user put fast-forward on the left trigger too: it yields with one warning.
    let mut user = InputConfig::default();
    binds(
        &mut user.hotkeys_gamepad,
        "fast_forward_hold",
        &["left_trigger", "left_stick"],
    );
    binds(&mut user.hotkeys_gamepad, "rewind_hold", &[]);
    let warnings = resolve_input_conflicts(&mut user, Some(PSX));
    assert_eq!(user.gamepad["l2"], ["left_trigger"]);
    assert_eq!(user.hotkeys_gamepad["fast_forward_hold"], ["left_stick"]);
    assert_eq!(
        user.gamepad["r2"],
        ["right_trigger"],
        "r2 had no hotkey left"
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("moved from `hotkeys_gamepad.fast_forward_hold`"));
    // Non-hold hotkeys still win over PS1 actions (save state on L2 by the user).
    let mut save = InputConfig::default();
    binds(&mut save.hotkeys_gamepad, "save_state", &["left_trigger"]);
    binds(&mut save.hotkeys_gamepad, "rewind_hold", &[]);
    let warnings = resolve_input_conflicts(&mut save, Some(PSX));
    assert!(save.gamepad["l2"].is_empty());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

#[test]
fn hotkey_wins_over_game_action_and_first_hotkey_wins() {
    let mut input = InputConfig::default();
    binds(&mut input.keyboard, "b", &["F1", "KeyZ"]);
    binds(&mut input.hotkeys_keyboard, "load_state", &["F1", "F4"]);
    binds(&mut input.gamepad, "a", &["right_trigger"]);
    binds(&mut input.gamepad, "y", &["east"]); // two actions on one input: allowed
    let warnings = resolve_input_conflicts(&mut input, None);
    assert_eq!(input.keyboard["b"], ["KeyZ"]);
    assert_eq!(input.hotkeys_keyboard["save_state"], ["F1"]);
    assert_eq!(input.hotkeys_keyboard["load_state"], ["F4"]);
    assert!(input.gamepad["a"].is_empty());
    assert_eq!(input.gamepad["y"], ["east"]);
    assert_eq!(
        input.hotkeys_gamepad["fast_forward_hold"],
        ["right_trigger"]
    );
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    assert!(warnings[0].contains("kept on `save_state`"), "{warnings:?}");
    assert!(warnings[1].contains("removed from `input_keyboard.b`"));
    assert!(warnings[2].contains("removed from `input_gamepad.a`"));
}

#[test]
fn devices_do_not_interact() {
    let mut input = InputConfig::default();
    // Same names never cross devices (keyboard and pad name spaces differ),
    // but a pad hotkey must not strip a keyboard action.
    binds(&mut input.hotkeys_gamepad, "save_state", &["south"]);
    let warnings = resolve_input_conflicts(&mut input, None);
    assert_eq!(input.keyboard, InputConfig::default().keyboard);
    assert!(input.gamepad["b"].is_empty());
    assert_eq!(warnings.len(), 1);
}

/// Dreamcast: analog triggers on `l2` / `r2`, the left stick, no select / l / r.
const DC: &[&str] = &[
    "b",
    "a",
    "y",
    "x",
    "start",
    "up",
    "down",
    "left",
    "right",
    "l2",
    "r2",
    "lstick_up",
    "lstick_down",
    "lstick_left",
    "lstick_right",
];

#[test]
fn dreamcast_triggers_are_game_buttons_and_the_keyboard_keeps_hold_hotkeys() {
    let mut input = InputConfig::default();
    assert!(resolve_input_conflicts(&mut input, Some(DC)).is_empty());
    assert_eq!(input.gamepad["l2"], ["left_trigger"], "LT");
    assert_eq!(input.gamepad["r2"], ["right_trigger"], "RT");
    assert!(input.hotkeys_gamepad["rewind_hold"].is_empty());
    assert!(input.hotkeys_gamepad["fast_forward_hold"].is_empty());
    assert!(!input.hotkeys_keyboard["rewind_hold"].is_empty());
    assert!(!input.hotkeys_keyboard["fast_forward_hold"].is_empty());
    assert!(input.keyboard["select"].is_empty(), "no Select on the pad");
    assert_eq!(input.keyboard["lstick_left"], ["KeyJ"]);
}
