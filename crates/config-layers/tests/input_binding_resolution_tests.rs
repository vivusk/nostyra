//! Binding, hotkey and resume keys across real layer files: defaults layer,
//! per-action inheritance, invalid values, conflicts and the flat views.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};

use config_layers::{
    layer_known_values, resolve_config, resolve_config_with_warnings, resolve_layers_with_warnings,
    resolved_known_values, set_layer_value, ConfigError, ConfigLayerPaths, InputConfig, ResumeMode,
    Value,
};

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("input-config-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn layers(root: &Path) -> ConfigLayerPaths {
    ConfigLayerPaths::new(
        root,
        "snes",
        "snes9x",
        Path::new("E:/ROMs/Game (USA).sfc"),
        "",
    )
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn partial_section_keeps_other_defaults_and_layers_override_per_action() {
    let root = scratch("partial");
    let paths = layers(&root);
    write(
        &paths.global,
        "[input_gamepad]\na = [\"north\"]\nx = [\"east\"]",
    );
    write(
        &paths.game,
        "[input_gamepad]\nx = [\"west\", \"left_stick\"]\n[input_keyboard]\nl = []",
    );
    let config = resolve_config(&paths, None);
    let defaults = InputConfig::default();
    let pad = &config.input.gamepad;
    assert_eq!(pad["a"], ["north"], "global");
    assert_eq!(pad["x"], ["west", "left_stick"], "game beats global");
    for action in [
        "b", "y", "select", "start", "up", "down", "left", "right", "l", "r",
    ] {
        assert_eq!(pad[action], defaults.gamepad[action], "{action} inherited");
    }
    assert!(config.input.keyboard["l"].is_empty(), "[] unbinds");
    assert_eq!(config.input.keyboard["r"], ["KeyW"], "absent key inherits");
    assert_eq!(config.input.keyboard.len(), 24);
}

/// A config written before the stick actions existed (every M9 action set, no
/// `lstick_*` / `rstick_*`): the new actions get their defaults merged in.
#[test]
fn old_config_without_stick_actions_gets_their_defaults() {
    let root = scratch("old-sticks");
    let paths = layers(&root);
    let old_keyboard: String = [
        "b", "y", "select", "start", "up", "down", "left", "right", "a", "x", "l", "r", "l2", "r2",
    ]
    .iter()
    .map(|action| format!("{action} = [\"KeyB\"]\n"))
    .collect();
    write(&paths.global, &format!("[input_keyboard]\n{old_keyboard}"));
    let input = resolve_config(&paths, None).input;
    assert_eq!(input.keyboard["b"], ["KeyB"], "user value kept");
    for (action, key) in [("lstick_up", "KeyI"), ("rstick_right", "KeyH")] {
        assert_eq!(input.keyboard[action], [key], "{action}");
    }
    for action in config_layers::PAD_ACTIONS {
        assert!(input.keyboard.contains_key(action), "{action}");
        assert!(input.gamepad.contains_key(action), "{action}");
    }
}

#[test]
fn invalid_binds_drop_only_that_key() {
    let root = scratch("invalid");
    let paths = layers(&root);
    write(
        &paths.global,
        "[input_keyboard]\nb = [\"KeyB\"]\n[hotkeys_gamepad]\nsave_state = [\"left_stick\"]",
    );
    write(
        &paths.game,
        "[input_keyboard]\nb = [\"Escape\"]\na = [\"KeyQ\", \"KeyW\", \"KeyE\"]\ny = \"KeyY\"\n\
         x = [\"KeyD\", \"KeyD\"]\nup = [\"Nope\"]\nturbo = [\"KeyT\"]\nleft = [\"KeyJ\"]\n\
         [input_gamepad]\nb = [\"guide\"]\nstart = [\"back\"]\n\
         [hotkeys_gamepad]\nsave_state = [\"start\"]\nload_state = [\"back\"]",
    );
    let input = resolve_config(&paths, None).input;
    let defaults = InputConfig::default();
    assert_eq!(input.keyboard["b"], ["KeyB"], "Escape → lower layer");
    for action in ["a", "y", "x", "up"] {
        assert_eq!(
            input.keyboard[action], defaults.keyboard[action],
            "{action}"
        );
    }
    assert_eq!(input.keyboard["left"], ["KeyJ"], "valid sibling kept");
    assert!(!input.keyboard.contains_key("turbo"));
    assert_eq!(input.gamepad["b"], ["south"], "guide rejected");
    assert_eq!(
        input.gamepad["start"],
        ["back"],
        "back is a valid game bind"
    );
    assert_eq!(input.hotkeys_gamepad["save_state"], ["left_stick"]);
    assert!(input.hotkeys_gamepad["load_state"].is_empty());
}

#[test]
fn conflicts_resolve_hotkey_first_with_warnings() {
    let root = scratch("conflicts");
    let paths = layers(&root);
    write(
        &paths.global,
        "[input_keyboard]\nb = [\"F1\"]\n[hotkeys_keyboard]\nnext_slot = [\"F2\"]",
    );
    let (config, warnings) = resolve_config_with_warnings(&paths, None);
    assert!(config.input.keyboard["b"].is_empty());
    assert_eq!(config.input.hotkeys_keyboard["save_state"], ["F1"]);
    assert_eq!(
        config.input.hotkeys_keyboard["prev_slot"],
        ["F2"],
        "first wins"
    );
    assert!(config.input.hotkeys_keyboard["next_slot"].is_empty());
    assert_eq!(warnings.len(), 2, "{warnings:?}");

    let (global_only, global_warnings) = resolve_layers_with_warnings(&[&paths.global], None);
    assert_eq!(global_only, config, "no other layer exists");
    assert_eq!(global_warnings, warnings);
    assert_eq!(
        resolve_config(&paths, None),
        config,
        "wrapper drops only the warnings"
    );
}

#[test]
fn resume_keys_layer_and_fall_back() {
    let root = scratch("resume");
    let paths = layers(&root);
    let defaults = resolve_config(&paths, None).resume;
    assert!(defaults.auto_save && defaults.mode == ResumeMode::Ask);
    write(
        &paths.global,
        "[resume]\nauto_save = false\nmode = \"always\"",
    );
    write(&paths.game, "[resume]\nmode = \"never\"");
    let resume = resolve_config(&paths, None).resume;
    assert!(!resume.auto_save);
    assert_eq!(resume.mode, ResumeMode::Never);
    write(&paths.global, "[resume]\nmode = \"sometimes\"");
    std::fs::write(&paths.game, "").unwrap();
    assert_eq!(resolve_config(&paths, None).resume.mode, ResumeMode::Ask);
}

#[test]
fn flat_views_carry_arrays_and_writer_validates_binds() {
    let root = scratch("views");
    let paths = layers(&root);
    let pad_a = |inputs: &[&str]| Value::Array(inputs.iter().map(|s| Value::from(*s)).collect());
    set_layer_value(&paths.game, "input_gamepad.a", Some(pad_a(&["north"]))).unwrap();
    let bad = set_layer_value(&paths.game, "input_gamepad.b", Some(pad_a(&["guide"])));
    assert!(matches!(bad, Err(ConfigError::InvalidValue { .. })));

    let layer = layer_known_values(&paths.game).unwrap();
    assert_eq!(layer.len(), 1);
    assert_eq!(layer["input_gamepad.a"], pad_a(&["north"]));
    let resolved = resolved_known_values(&resolve_config(&paths, None));
    assert_eq!(resolved["input_gamepad.a"], pad_a(&["north"]));
    assert_eq!(resolved["hotkeys_gamepad.pause_menu"], pad_a(&[]));
    assert_eq!(resolved["resume.mode"], Value::from("ask"));
}
