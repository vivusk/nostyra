//! Steal planning against the built-in defaults (pure, no files).

use super::*;

fn arr(names: &[&str]) -> Option<Value> {
    Some(Value::Array(
        names.iter().map(|n| Value::from(*n)).collect(),
    ))
}

/// Built-in defaults as a view sees them (conflicts resolved: the hold hotkeys
/// keep the triggers outside a PlayStation game).
fn defaults() -> InputConfig {
    let mut input = InputConfig::default();
    config_layers::resolve_input_conflicts(&mut input, None);
    input
}

#[test]
fn keyboard_z_moves_from_b_to_a() {
    let plan = plan_assign(
        &defaults(),
        InputDevice::Keyboard,
        "input_keyboard.a",
        "KeyZ",
    )
    .unwrap();
    assert_eq!(
        plan.writes,
        vec![
            ("input_keyboard.a".to_string(), arr(&["KeyX", "KeyZ"])),
            ("input_keyboard.b".to_string(), arr(&[])),
        ]
    );
    assert_eq!(plan.removed_from, ["input_keyboard.b"]);
}

#[test]
fn hotkey_input_moves_to_a_game_action() {
    // F1 = save_state by default.
    let plan = plan_assign(&defaults(), InputDevice::Keyboard, "input_keyboard.x", "F1").unwrap();
    assert_eq!(
        plan.writes[0],
        ("input_keyboard.x".into(), arr(&["KeyS", "F1"]))
    );
    assert_eq!(plan.removed_from, ["hotkeys_keyboard.save_state"]);
    assert_eq!(
        plan.writes[1],
        ("hotkeys_keyboard.save_state".into(), arr(&[]))
    );
}

#[test]
fn gamepad_steal_stays_on_its_device() {
    // right_trigger: pad fast-forward; keyboard sections untouched.
    let plan = plan_assign(
        &defaults(),
        InputDevice::Gamepad,
        "input_gamepad.r",
        "right_trigger",
    )
    .unwrap();
    assert_eq!(plan.removed_from, ["hotkeys_gamepad.fast_forward_hold"]);
    assert!(plan.writes.iter().all(|(key, _)| key.contains("gamepad")));
    // Pad hotkeys may use L3 but never back/start/guide.
    assert!(plan_assign(
        &defaults(),
        InputDevice::Gamepad,
        "hotkeys_gamepad.save_state",
        "left_stick"
    )
    .is_ok());
}

#[test]
fn already_bound_input_is_a_no_op_on_the_target() {
    let plan = plan_assign(
        &defaults(),
        InputDevice::Keyboard,
        "input_keyboard.b",
        "KeyZ",
    )
    .unwrap();
    assert_eq!(
        plan.writes,
        vec![("input_keyboard.b".to_string(), arr(&["KeyZ"]))]
    );
    assert!(plan.removed_from.is_empty());
}

#[test]
fn invalid_inputs_and_targets_are_refused() {
    let cases = [
        (InputDevice::Keyboard, "input_keyboard.a", "Escape"),
        (InputDevice::Gamepad, "input_gamepad.a", "guide"),
        (InputDevice::Gamepad, "hotkeys_gamepad.save_state", "back"),
        (InputDevice::Gamepad, "hotkeys_gamepad.load_state", "start"),
        (InputDevice::Keyboard, "input_keyboard.a", "KeyÄ"),
        (InputDevice::Keyboard, "input_keyboard.a", "south"),
        (InputDevice::Gamepad, "input_gamepad.a", "KeyZ"),
        (InputDevice::Keyboard, "input_gamepad.a", "KeyZ"),
        (InputDevice::Keyboard, "input_keyboard.turbo", "KeyZ"),
        (InputDevice::Keyboard, "video.shader", "KeyZ"),
        (InputDevice::Keyboard, "input_keyboard", "KeyZ"),
    ];
    for (device, target, input) in cases {
        let err = plan_assign(&defaults(), device, target, input).unwrap_err();
        assert_eq!(err.kind, "invalid", "{target} {input}");
    }
    // back/start stay bindable as game actions.
    assert!(plan_assign(
        &defaults(),
        InputDevice::Gamepad,
        "input_gamepad.a",
        "start"
    )
    .is_ok());
}

#[test]
fn third_bind_is_refused() {
    let mut config = defaults();
    config
        .keyboard
        .insert("a".into(), vec!["KeyX".into(), "KeyC".into()]);
    let err = plan_assign(&config, InputDevice::Keyboard, "input_keyboard.a", "KeyV").unwrap_err();
    assert_eq!(err.kind, "invalid");
}

#[test]
fn input_on_two_actions_is_taken_from_both() {
    let mut config = defaults();
    config
        .keyboard
        .insert("y".into(), vec!["KeyA".into(), "KeyZ".into()]);
    let plan = plan_assign(
        &config,
        InputDevice::Keyboard,
        "hotkeys_keyboard.pause_menu",
        "KeyZ",
    )
    .unwrap();
    assert_eq!(plan.removed_from, ["input_keyboard.b", "input_keyboard.y"]);
    assert!(plan
        .writes
        .contains(&("input_keyboard.y".to_string(), arr(&["KeyA"]))));
}

#[test]
fn escape_and_guide_are_refused_on_every_target() {
    let sections = [
        (
            InputDevice::Keyboard,
            INPUT_KEYBOARD_SECTION,
            &PAD_ACTIONS[..],
            "Escape",
        ),
        (
            InputDevice::Keyboard,
            HOTKEYS_KEYBOARD_SECTION,
            &HOTKEYS[..],
            "Escape",
        ),
        (
            InputDevice::Gamepad,
            INPUT_GAMEPAD_SECTION,
            &PAD_ACTIONS[..],
            "guide",
        ),
        (
            InputDevice::Gamepad,
            HOTKEYS_GAMEPAD_SECTION,
            &HOTKEYS[..],
            "guide",
        ),
    ];
    for (device, section, keys, fixed) in sections {
        for key in keys {
            let target = format!("{section}.{key}");
            let err = plan_assign(&defaults(), device, &target, fixed).unwrap_err();
            assert_eq!(err.kind, "invalid", "{target} {fixed}");
        }
    }
}
