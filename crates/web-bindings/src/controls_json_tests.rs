use config_layers::Value;

use super::*;

fn some(text: &str) -> Option<String> {
    Some(text.to_string())
}

fn arr(names: &[&str]) -> Value {
    Value::Array(names.iter().map(|n| Value::from(*n)).collect())
}

#[test]
fn global_view_ignores_game_layers_and_splits_controls() {
    let layers = [
        some("[video]\nscaling = \"integer\"\n[input_keyboard]\nb = [\"KeyC\"]\n"),
        some("[video]\nscaling = \"stretch\"\n"),
        None,
        some("[resume]\nmode = \"never\"\n"),
    ];
    let global = settings_view(None, &layers).view;
    assert_eq!(global.resolved["video.scaling"], Value::from("integer"));
    assert_eq!(global.input.global["input_keyboard.b"], arr(&["KeyC"]));
    assert!(global.game.is_empty() && global.input.game.is_empty());

    let game = settings_view(Some("snes"), &layers).view;
    assert_eq!(game.resolved["video.scaling"], Value::from("stretch"));
    assert_eq!(game.game["resume.mode"], Value::from("never"));
    assert_eq!(game.input.resolved["input_keyboard.b"], arr(&["KeyC"]));
}

#[test]
fn assign_steals_in_the_scope_layer_and_keeps_unknown_keys() {
    let layers = [some("extra = 1\n"), None, None, None];
    let out = assign(None, &layers, "keyboard", "input_keyboard.a", "KeyZ").unwrap();
    assert_eq!(out.removed_from, ["input_keyboard.b"]);
    assert!(out.text.contains("extra = 1"), "{}", out.text);
    let values = crate::config_json::layer_values(Some(&out.text)).unwrap();
    assert_eq!(values["input_keyboard.a"], arr(&["KeyX", "KeyZ"]));
    assert_eq!(values["input_keyboard.b"], arr(&[]));

    // Game scope: the game layer is edited, the global one only read.
    let layers = [some(&out.text), None, None, None];
    let game = assign(Some("snes"), &layers, "gamepad", "input_gamepad.a", "south").unwrap();
    assert_eq!(game.removed_from, ["input_gamepad.b"]);
    assert!(!game.text.contains("input_keyboard"), "{}", game.text);

    assert!(assign(None, &layers, "keyboard", "input_keyboard.a", "Escape").is_err());
    assert!(assign(None, &layers, "mouse", "input_keyboard.a", "KeyQ").is_err());
}

#[test]
fn single_key_writes_and_reset() {
    let edit = |key: &str, value| KeyEdit {
        key: key.into(),
        value,
    };
    let text = set_known_value(
        None,
        edit("resume.mode", Some(serde_json::json!("always"))),
        false,
    )
    .unwrap();
    assert!(text.contains("mode = \"always\""), "{text}");
    assert!(set_known_value(None, edit("achievements.enabled", None), true).is_err());
    assert!(set_known_value(None, edit("video.future", None), false).is_err());

    let with_binds = format!("{text}[input_gamepad]\na = [\"north\"]\n[extra]\nkept = 1\n");
    let reset = reset_controls(Some(&with_binds)).unwrap();
    assert!(
        !reset.contains("input_gamepad") && reset.contains("kept = 1"),
        "{reset}"
    );
    assert!(reset.contains("always"), "{reset}");
}

#[test]
fn catalog_lists_every_action() {
    let json = serde_json::to_value(catalog()).unwrap();
    assert_eq!(json["padActions"].as_array().unwrap().len(), 24);
    assert_eq!(
        json["defaults"]["hotkeys_keyboard"]["save_state"],
        serde_json::json!(["F1"])
    );
}
