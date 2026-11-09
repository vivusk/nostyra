//! PlayStation 2 pad (DualShock 2): the pad's LT / RT hold hotkeys (rewind /
//! fast-forward) yield to L2 / R2, the stick clicks are L3 / R3 game buttons, every
//! RetroPad action is used so nothing is dropped, keyboard hotkeys never yield.

use super::*;

/// The DualShock 2 as the registry lists it (`system_catalog::PS2.pad_actions`).
#[rustfmt::skip]
const PS2: &[&str] = &[
    "b", "a", "y", "x", "select", "start", "up", "down", "left", "right", "l", "r", "l2", "r2",
    "l3", "r3",
    "lstick_up", "lstick_down", "lstick_left", "lstick_right",
    "rstick_up", "rstick_down", "rstick_left", "rstick_right",
];

#[test]
fn ps2_uses_every_retropad_action() {
    assert_eq!(PS2.len(), PAD_ACTIONS.len());
    for action in PAD_ACTIONS {
        assert!(PS2.contains(&action), "{action}");
    }
}

#[test]
fn both_triggers_yield_to_l2_and_r2_and_the_stick_clicks_stay() {
    let mut input = InputConfig::default();
    let warnings = resolve_input_conflicts(&mut input, Some(PS2));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(input.gamepad["l2"], ["left_trigger"]);
    assert_eq!(input.gamepad["r2"], ["right_trigger"]);
    assert!(input.hotkeys_gamepad["rewind_hold"].is_empty());
    assert!(input.hotkeys_gamepad["fast_forward_hold"].is_empty());
    assert_eq!(input.gamepad["l3"], ["left_stick"]);
    assert_eq!(input.gamepad["r3"], ["right_stick"]);
    assert_eq!(input.gamepad["l"], ["left_shoulder"], "L1");
    assert_eq!(input.gamepad["select"], ["back"]);
    // Nothing is dropped: every default pad bind of a used action survives.
    for (action, defaults) in DEFAULT_INPUT_GAMEPAD {
        assert_eq!(input.gamepad[*action], *defaults, "{action}");
    }
    // Keyboard hotkeys never yield; L3 / R3 start unbound there (as on the DS).
    assert_eq!(input.hotkeys_keyboard["fast_forward_hold"], ["Backquote"]);
    assert_eq!(input.hotkeys_keyboard["rewind_hold"], ["Backspace"]);
    assert!(input.keyboard["l3"].is_empty() && input.keyboard["r3"].is_empty());
    assert_eq!(input.keyboard["lstick_up"], ["KeyI"]);
    assert_eq!(input.keyboard["rstick_up"], ["KeyT"]);
}

#[test]
fn a_user_moved_hold_hotkey_still_yields_with_a_warning() {
    let mut input = InputConfig::default();
    input
        .hotkeys_gamepad
        .insert("rewind_hold".into(), vec!["right_trigger".into()]);
    input
        .hotkeys_gamepad
        .insert("fast_forward_hold".into(), Vec::new());
    let warnings = resolve_input_conflicts(&mut input, Some(PS2));
    assert_eq!(input.gamepad["r2"], ["right_trigger"]);
    assert!(input.hotkeys_gamepad["rewind_hold"].is_empty());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("rewind_hold") && warnings[0].contains("r2"));
}
