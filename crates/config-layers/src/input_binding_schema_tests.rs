use std::collections::HashSet;

use super::*;
use crate::input_binding_names::{gamepad_input_label, GAMEPAD_INPUT_LABELS};
use crate::known_key_checks::check_known_key;
use crate::resolved_config_schema::{to_resolved, ResolvedConfig};

fn unique(names: &[&str]) -> bool {
    names.iter().collect::<HashSet<_>>().len() == names.len()
}

#[test]
fn name_tables_are_unique_and_hold_the_reserved_inputs() {
    assert!(unique(&PAD_ACTIONS) && unique(&HOTKEYS));
    assert!(unique(KEYBOARD_KEY_NAMES) && unique(&GAMEPAD_INPUT_NAMES));
    assert!(
        KEYBOARD_KEY_NAMES.len() >= 100,
        "{}",
        KEYBOARD_KEY_NAMES.len()
    );
    assert!(RESERVED_KEYBOARD_KEYS
        .iter()
        .all(|k| KEYBOARD_KEY_NAMES.contains(k)));
    assert!(RESERVED_GAMEPAD_INPUTS
        .iter()
        .chain(&RESERVED_PAD_HOTKEY_INPUTS)
        .all(|k| GAMEPAD_INPUT_NAMES.contains(k)));
}

#[test]
fn every_gamepad_input_has_xbox_and_sony_labels() {
    let labelled: Vec<&str> = GAMEPAD_INPUT_LABELS.iter().map(|(n, _)| *n).collect();
    assert_eq!(labelled, GAMEPAD_INPUT_NAMES, "same names, same order");
    for name in GAMEPAD_INPUT_NAMES {
        let (xbox, sony) = gamepad_input_label(name).unwrap();
        assert!(!xbox.is_empty() && !sony.is_empty(), "{name}");
    }
    assert_eq!(gamepad_input_label("south"), Some(("A", "Cross")));
    assert_eq!(gamepad_input_label("back"), Some(("View", "Create")));
    assert_eq!(gamepad_input_label("nope"), None);
}

#[test]
fn defaults_cover_every_key_and_pass_their_checks() {
    for (section, keys, _, defaults) in &BINDING_SECTIONS {
        let names: Vec<&str> = defaults.iter().map(|(k, _)| *k).collect();
        assert_eq!(names, *keys, "{section}: every key, in order");
        for (key, inputs) in defaults.iter() {
            let value = Value::Array(inputs.iter().map(|s| Value::from(*s)).collect());
            assert_eq!(
                check_known_key(&format!("{section}.{key}"), &value),
                Some(true),
                "{section}.{key}"
            );
        }
    }
}

#[test]
fn defaults_table_resolves_to_the_default_config() {
    let config = to_resolved(defaults_table());
    assert_eq!(config, ResolvedConfig::default());
    assert_eq!(config.input, InputConfig::default());
}

/// Today's (M1–M3) layout, table-driven.
#[test]
fn defaults_are_the_m3_bindings() {
    let input = InputConfig::default();
    for (action, key) in [
        ("b", "KeyZ"),
        ("a", "KeyX"),
        ("y", "KeyA"),
        ("x", "KeyS"),
        ("l", "KeyQ"),
        ("r", "KeyW"),
        ("start", "Enter"),
        ("select", "ShiftRight"),
        ("up", "ArrowUp"),
    ] {
        assert_eq!(input.keyboard[action], [key], "{action}");
    }
    for (action, pad) in [
        ("b", "south"),
        ("a", "east"),
        ("select", "back"),
        ("l", "left_shoulder"),
    ] {
        assert_eq!(input.gamepad[action], [pad], "{action}");
    }
    for (hotkey, key) in [
        ("save_state", "F1"),
        ("load_state", "F4"),
        ("prev_slot", "F2"),
        ("next_slot", "F3"),
        ("toggle_fullscreen", "F11"),
        ("pause_menu", "F10"),
        ("fast_forward_hold", "Backquote"),
        ("rewind_hold", "Backspace"),
    ] {
        assert_eq!(input.hotkeys_keyboard[hotkey], [key], "{hotkey}");
    }
    assert_eq!(
        input.hotkeys_gamepad["fast_forward_hold"],
        ["right_trigger"]
    );
    assert_eq!(input.hotkeys_gamepad["rewind_hold"], ["left_trigger"]);
    assert!(
        input.hotkeys_gamepad["pause_menu"].is_empty(),
        "Guide is fixed"
    );
    assert_eq!(input.hotkeys_gamepad.len(), HOTKEYS.len());
}

#[test]
fn unknown_binding_keys_are_dropped_elsewhere_kept() {
    let mut layer: Table =
        toml::from_str("[input_keyboard]\nb = [\"KeyZ\"]\nturbo = [\"KeyT\"]\n[video]\nfuture = 1")
            .unwrap();
    drop_unknown_binding_keys(&mut layer, Path::new("global.toml"));
    let keyboard = layer["input_keyboard"].as_table().unwrap();
    assert!(keyboard.contains_key("b") && !keyboard.contains_key("turbo"));
    assert!(layer["video"].as_table().unwrap().contains_key("future"));
}

#[test]
fn escape_and_guide_are_rejected_in_every_binding_section() {
    for (section, keys, _, _) in &BINDING_SECTIONS {
        let fixed = if section.ends_with("keyboard") {
            "Escape"
        } else {
            "guide"
        };
        for key in keys.iter() {
            let value = Value::Array(vec![Value::from(fixed)]);
            assert_eq!(
                check_known_key(&format!("{section}.{key}"), &value),
                Some(false),
                "{section}.{key} = [{fixed}]"
            );
        }
    }
}
