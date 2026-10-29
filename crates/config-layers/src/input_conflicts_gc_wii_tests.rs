//! GameCube / Wii pads: the pad's LT / RT hold hotkeys (rewind / fast-forward)
//! yield to L2 / R2 (GC analog L / R; Wii Nunchuk / Remote shake), the stick clicks
//! stay game buttons, unused actions are dropped silently.

use super::*;

/// The GameCube pad as the registry lists it (`system_catalog::GC.pad_actions`).
#[rustfmt::skip]
const GC: &[&str] = &[
    "a", "b", "x", "y", "start", "up", "down", "left", "right", "r", "l2", "r2", "l3", "r3",
    "lstick_up", "lstick_down", "lstick_left", "lstick_right",
    "rstick_up", "rstick_down", "rstick_left", "rstick_right",
];
/// The Wii Remote + Nunchuk as the registry lists it (`system_catalog::WII`).
#[rustfmt::skip]
const WII: &[&str] = &[
    "a", "b", "x", "y", "select", "start", "up", "down", "left", "right", "l", "r", "l2", "r2",
    "r3",
    "lstick_up", "lstick_down", "lstick_left", "lstick_right",
    "rstick_up", "rstick_down", "rstick_left", "rstick_right",
];

#[test]
fn both_triggers_yield_to_l2_and_r2_silently() {
    for (name, actions) in [("gc", GC), ("wii", WII)] {
        let mut input = InputConfig::default();
        let warnings = resolve_input_conflicts(&mut input, Some(actions));
        assert!(warnings.is_empty(), "{name}: {warnings:?}");
        assert_eq!(input.gamepad["l2"], ["left_trigger"], "{name}");
        assert_eq!(input.gamepad["r2"], ["right_trigger"], "{name}");
        assert!(input.hotkeys_gamepad["rewind_hold"].is_empty(), "{name}");
        assert!(
            input.hotkeys_gamepad["fast_forward_hold"].is_empty(),
            "{name}"
        );
        assert_eq!(input.gamepad["r3"], ["right_stick"], "{name}");
        // Keyboard hotkeys never yield.
        assert_eq!(input.hotkeys_keyboard["fast_forward_hold"], ["Backquote"]);
        assert_eq!(input.hotkeys_keyboard["rewind_hold"], ["Backspace"]);
    }
}

#[test]
fn unused_actions_are_dropped_per_system() {
    let mut gc = InputConfig::default();
    assert!(resolve_input_conflicts(&mut gc, Some(GC)).is_empty());
    assert_eq!(gc.gamepad["l3"], ["left_stick"], "L light press");
    assert_eq!(gc.gamepad["r"], ["right_shoulder"], "Z");
    assert!(gc.gamepad["select"].is_empty() && gc.gamepad["l"].is_empty());
    let mut wii = InputConfig::default();
    assert!(resolve_input_conflicts(&mut wii, Some(WII)).is_empty());
    assert!(wii.gamepad["l3"].is_empty(), "sideways toggle disabled");
    assert_eq!(wii.gamepad["select"], ["back"], "2");
    assert_eq!(wii.gamepad["l"], ["left_shoulder"], "minus");
}

#[test]
fn a_user_moved_hold_hotkey_still_yields_with_a_warning() {
    let mut input = InputConfig::default();
    input
        .hotkeys_gamepad
        .insert("fast_forward_hold".into(), vec!["left_trigger".into()]);
    input
        .hotkeys_gamepad
        .insert("rewind_hold".into(), Vec::new());
    let warnings = resolve_input_conflicts(&mut input, Some(WII));
    assert_eq!(input.gamepad["l2"], ["left_trigger"], "Shake Nunchuk");
    assert!(input.hotkeys_gamepad["fast_forward_hold"].is_empty());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}
