//! Config command logic against a scratch data dir (no Tauri runtime needed).

use config_layers::resolve_config;

use super::*;
use crate::commands::input_config_view::is_input_key;

fn scratch(name: &str) -> tempfile::TempDir {
    let prefix = format!("nostyra-gpcfg-{name}-");
    tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
}

fn game(rom: &str) -> GameDataTarget {
    GameDataTarget {
        system: "snes".into(),
        rom_path: rom.into(),
        content_sha1: "6b47bb75d16514b6a476aa0c73a683a2a4c18765".into(),
        missing: false,
    }
}

#[test]
fn game_layer_write_lands_at_named_file_and_resolves() {
    let temp = scratch("game");
    let root = temp.path();
    let layers = game("E:/ROMs/Super Metroid (USA).sfc").config_layers(root);
    write_known_value(
        &layers.game,
        "video.shader",
        Some(Value::String("crt-geom".into())),
    )
    .unwrap();
    let expected = root
        .join("config")
        .join("game")
        .join("snes")
        .join("Super Metroid (USA) [6B47BB75].toml");
    assert_eq!(layers.game, expected);
    assert!(expected.is_file());
    assert_eq!(resolve_config(&layers, None).video.shader, "crt-geom");
    assert_eq!(
        layer_view(&layers.game)["video.shader"],
        Value::String("crt-geom".into())
    );

    // null = "use default": the key goes, the global value shows through.
    let global = global_config_path(root);
    write_known_value(
        &global,
        "video.shader",
        Some(Value::String("crt-lottes".into())),
    )
    .unwrap();
    write_known_value(&layers.game, "video.shader", None).unwrap();
    assert!(layer_view(&layers.game).is_empty());
    assert_eq!(resolve_config(&layers, None).video.shader, "crt-lottes");
}

#[test]
fn unknown_key_or_bad_value_is_invalid_and_writes_nothing() {
    let temp = scratch("invalid");
    let root = temp.path();
    let path = global_config_path(root);
    for (key, value) in [
        ("video.unknown", Value::Boolean(true)),
        ("shader", Value::String("none".into())),
        ("latency.run_ahead_frames", Value::Integer(3)),
        ("rewind.buffer_mb", Value::Integer(8)),
        ("video.shader", Value::String("crt-royale".into())),
        ("video.scaling", Value::Integer(1)),
    ] {
        let err = write_known_value(&path, key, Some(value)).unwrap_err();
        assert_eq!(err.kind, "invalid", "{key}: {}", err.message);
    }
    assert!(!path.exists());
}

#[test]
fn ipc_arguments_deserialize_from_the_ui_shapes() {
    let global: ConfigScope = serde_json::from_str("\"global\"").unwrap();
    assert!(matches!(global, ConfigScope::Global(GlobalScope::Global)));
    let game: ConfigScope = serde_json::from_str("{\"gameId\": 4}").unwrap();
    assert!(matches!(game, ConfigScope::Game { game_id: 4 }));
    assert!(serde_json::from_str::<ConfigScope>("\"system\"").is_err());
    // JS numbers/bools/strings/null → TOML values the schema checks accept.
    let value: Option<Value> = serde_json::from_str("1").unwrap();
    assert_eq!(value, Some(Value::Integer(1)));
    let cleared: Option<Value> = serde_json::from_str("null").unwrap();
    assert_eq!(cleared, None);
}

#[test]
fn view_keeps_controls_out_of_the_gameplay_maps() {
    let temp = scratch("view");
    let root = temp.path();
    let global = global_config_path(root);
    let layers = game("E:/ROMs/View.sfc").config_layers(root);
    // Hand-made global conflict: F1 on save_state (default) and on game button `b`.
    std::fs::create_dir_all(global.parent().unwrap()).unwrap();
    std::fs::write(
        &global,
        "[resume]\nmode = \"always\"\n[input_keyboard]\nb = [\"F1\"]\n",
    )
    .unwrap();
    write_known_value(
        &layers.game,
        "input_gamepad.a",
        Some(Value::Array(vec![Value::from("north")])),
    )
    .unwrap();
    write_known_value(
        &layers.game,
        "resume.auto_save",
        Some(Value::Boolean(false)),
    )
    .unwrap();

    let no_game = build_view(&global, None, None);
    for map in [&no_game.resolved, &no_game.global] {
        assert!(map.keys().all(|key| !is_input_key(key)), "{map:?}");
    }
    assert_eq!(no_game.resolved["resume.mode"], Value::from("always"));
    assert_eq!(no_game.global.len(), 1);
    assert_eq!(no_game.input.global.len(), 1);
    // Resolved (hotkey wins), not `defaults ∪ raw global`.
    assert_eq!(
        no_game.input.resolved["input_keyboard.b"],
        Value::Array(vec![])
    );
    assert_eq!(
        no_game.input.warnings.len(),
        1,
        "{:?}",
        no_game.input.warnings
    );
    assert_eq!(
        no_game.input.resolved.len(),
        66,
        "2 × 24 actions + 2 × 9 hotkeys"
    );

    let with_game = build_view(
        &global,
        Some(&layers),
        system_catalog::system("snes").map(|s| s.pad_actions),
    );
    assert_eq!(
        with_game.game.keys().collect::<Vec<_>>(),
        ["resume.auto_save"]
    );
    assert_eq!(
        with_game.input.game.keys().collect::<Vec<_>>(),
        ["input_gamepad.a"]
    );
    assert_eq!(
        with_game.resolved.len(),
        12,
        "incl. the 5 global-only achievements keys"
    );
    assert_eq!(
        with_game.input.resolved["input_gamepad.a"],
        Value::Array(vec![Value::from("north")])
    );
    // A binding key's layer write is validated like any other key.
    let err = write_known_value(
        &global,
        "input_keyboard.a",
        Some(Value::Array(vec![Value::from("Escape")])),
    )
    .unwrap_err();
    assert_eq!(err.kind, "invalid");
}

#[test]
fn achievements_keys_are_refused_for_a_game_and_hidden_from_its_layer() {
    let game = ConfigScope::Game { game_id: 4 };
    let err = refuse_global_only_in_game(&game, "achievements.enabled").unwrap_err();
    assert_eq!(err.kind, "invalid");
    assert!(refuse_global_only_in_game(&game, "video.shader").is_ok());
    assert!(refuse_global_only_in_game(
        &ConfigScope::Global(GlobalScope::Global),
        "achievements.enabled"
    )
    .is_ok());
    let mut layer = Table::new();
    layer.insert("achievements.sound".into(), Value::Boolean(false));
    let (gameplay, input) = split_game_layer(layer);
    assert!(gameplay.is_empty() && input.is_empty());
}
