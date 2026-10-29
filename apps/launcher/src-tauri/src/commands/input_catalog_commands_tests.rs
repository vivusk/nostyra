//! Catalog, assign and reset against scratch layer files.

use config_layers::{
    global_config_path, layer_known_values, resolve_config, resolve_layers_with_warnings,
    set_layer_value, GAMEPAD_INPUT_NAMES,
};

use super::*;
use crate::commands::game_data_target::GameDataTarget;

fn scratch(name: &str) -> tempfile::TempDir {
    let prefix = format!("nostyra-ctl-{name}-");
    tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
}

fn arr(names: &[&str]) -> Value {
    Value::Array(names.iter().map(|n| Value::from(*n)).collect())
}

#[test]
fn catalog_comes_from_config_layers() {
    let catalog = get_input_catalog();
    assert_eq!(catalog.pad_actions.len(), 24);
    assert_eq!(catalog.hotkeys.len(), 9);
    assert_eq!(catalog.gamepad_inputs.len(), GAMEPAD_INPUT_NAMES.len());
    assert_eq!(catalog.defaults["input_keyboard"]["b"], arr(&["KeyZ"]));
    let json = serde_json::to_value(&catalog).unwrap();
    for field in [
        "padActions",
        "keyboardKeys",
        "reservedGamepadInputs",
        "maxBinds",
        "defaults",
    ] {
        assert!(json.get(field).is_some(), "{field}");
    }
    assert_eq!(
        json["gamepadInputs"][0],
        serde_json::json!({"name": "south", "xbox": "A", "sony": "Cross"})
    );
}

#[test]
fn game_scope_steal_writes_the_inherited_holder_remainder() {
    let temp = scratch("steal");
    let root = temp.path();
    let game = GameDataTarget {
        system: "snes".into(),
        rom_path: "E:/ROMs/Steal.sfc".into(),
        content_sha1: String::new(),
        missing: false,
    };
    let layers = game.config_layers(root);
    // Global: y = [KeyA, KeyC]; the game steals KeyC for `a`.
    set_layer_value(
        &layers.global,
        "input_keyboard.y",
        Some(arr(&["KeyA", "KeyC"])),
    )
    .unwrap();
    let config = resolve_config(&layers, None);
    let removed = assign_in_layer(
        &layers.game,
        &config,
        InputDevice::Keyboard,
        "input_keyboard.a",
        "KeyC",
    )
    .unwrap();
    assert_eq!(removed, ["input_keyboard.y"]);
    let game_layer = layer_known_values(&layers.game).unwrap();
    assert_eq!(game_layer["input_keyboard.a"], arr(&["KeyX", "KeyC"]));
    assert_eq!(game_layer["input_keyboard.y"], arr(&["KeyA"]));
    assert_eq!(game_layer.len(), 2, "{game_layer:?}");
    let after = resolve_config(&layers, None);
    assert_eq!(after.input.keyboard["y"], ["KeyA"]);
    // Global layer untouched.
    assert_eq!(layer_known_values(&layers.global).unwrap().len(), 1);
}

#[test]
fn invalid_assign_writes_nothing() {
    let temp = scratch("invalid");
    let root = temp.path();
    let global = global_config_path(root);
    let (config, _) = resolve_layers_with_warnings(&[&global], None);
    let err = assign_in_layer(
        &global,
        &config,
        InputDevice::Keyboard,
        "input_keyboard.a",
        "Escape",
    )
    .unwrap_err();
    assert_eq!(err.kind, "invalid");
    assert!(!global.exists());
}

#[test]
fn reset_controls_removes_only_binding_keys() {
    let temp = scratch("reset");
    let root = temp.path();
    let global = global_config_path(root);
    std::fs::create_dir_all(global.parent().unwrap()).unwrap();
    std::fs::write(
        &global,
        "[video]\nshader = \"crt-geom\"\n[resume]\nmode = \"never\"\n\
         [input_keyboard]\nb = [\"KeyC\"]\n[hotkeys_gamepad]\nsave_state = [\"left_stick\"]\n\
         [extra]\nkept = 1\n",
    )
    .unwrap();
    reset_controls_in(&global).unwrap();
    let view = layer_known_values(&global).unwrap();
    assert_eq!(
        view.keys().collect::<Vec<_>>(),
        ["resume.mode", "video.shader"]
    );
    assert!(std::fs::read_to_string(&global)
        .unwrap()
        .contains("kept = 1"));
}
