//! Nintendo DS pad: RT fast-forward yields to R2 (touch), LT rewind stays, the
//! stick clicks are L3 / R3, and systems without them drop them silently.

use super::*;

/// The DS pad as the registry lists it (`system_catalog::NDS.pad_actions`).
#[rustfmt::skip]
const NDS: &[&str] = &[
    "b", "a", "y", "x", "select", "start", "up", "down", "left", "right", "l", "r", "r2",
    "l3", "r3", "rstick_up", "rstick_down", "rstick_left", "rstick_right",
];
const SNES: &[&str] = &[
    "b", "y", "select", "start", "up", "down", "left", "right", "a", "x", "l", "r",
];

#[test]
fn nds_right_trigger_touches_and_left_trigger_keeps_rewind() {
    let mut input = InputConfig::default();
    assert!(resolve_input_conflicts(&mut input, Some(NDS)).is_empty());
    assert_eq!(input.gamepad["r2"], ["right_trigger"], "touch");
    assert!(input.hotkeys_gamepad["fast_forward_hold"].is_empty());
    assert_eq!(input.hotkeys_gamepad["rewind_hold"], ["left_trigger"]);
    assert!(input.gamepad["l2"].is_empty(), "lid not exposed");
    assert_eq!(input.gamepad["l3"], ["left_stick"], "microphone");
    assert_eq!(input.gamepad["r3"], ["right_stick"], "swap screens");
    // Keyboard hotkeys are unchanged; L3 / R3 start unbound there.
    assert_eq!(input.hotkeys_keyboard["fast_forward_hold"], ["Backquote"]);
    assert_eq!(input.keyboard["r2"], ["KeyR"]);
    assert!(input.keyboard["l3"].is_empty() && input.keyboard["r3"].is_empty());
    assert_eq!(input.keyboard["rstick_up"], ["KeyT"], "cursor keys");
    assert!(
        input.keyboard["lstick_up"].is_empty(),
        "no left-stick actions"
    );
}

#[test]
fn stick_clicks_are_dropped_silently_where_unused_and_bindable_on_keys() {
    let mut snes = InputConfig::default();
    assert!(resolve_input_conflicts(&mut snes, Some(SNES)).is_empty());
    assert!(snes.gamepad["l3"].is_empty() && snes.gamepad["r3"].is_empty());
    assert_eq!(snes.hotkeys_gamepad["fast_forward_hold"], ["right_trigger"]);
    let mut none = InputConfig::default();
    assert!(resolve_input_conflicts(&mut none, None).is_empty());
    assert_eq!(
        none.gamepad["r3"],
        ["right_stick"],
        "no-game view keeps them"
    );
    // A user key on L3 stays; a key also on a hotkey loses it, with a warning.
    let mut user = InputConfig::default();
    user.keyboard
        .insert("l3".into(), vec!["KeyM".into(), "F1".into()]);
    let warnings = resolve_input_conflicts(&mut user, Some(NDS));
    assert_eq!(user.keyboard["l3"], ["KeyM"]);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}
