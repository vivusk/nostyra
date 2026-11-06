use super::*;

#[test]
fn layers_merge_lowest_first_and_bad_layers_are_skipped() {
    let layers = [
        Some("[video]\nshader = \"crt-geom\"\n".to_string()),
        Some("not toml [".to_string()),
        None,
        Some("[video]\nshader = \"none\"\n[rewind]\nbuffer_mb = 9999\n".to_string()),
    ];
    let view = resolve(Some("snes"), &layers);
    assert_eq!(
        view.values["video.shader"],
        Value::from("none"),
        "game wins"
    );
    assert_eq!(
        view.values["rewind.buffer_mb"],
        Value::Integer(64),
        "invalid value dropped → default"
    );
    assert!(
        view.warnings
            .iter()
            .any(|w| w.contains("config/system.toml")),
        "{:?}",
        view.warnings
    );
}

#[test]
fn edits_round_trip_through_text() {
    let edits = [
        KeyEdit {
            key: "video.shader".into(),
            value: Some(serde_json::json!("crt-geom")),
        },
        KeyEdit {
            key: "input_keyboard.a".into(),
            value: Some(serde_json::json!(["KeyX"])),
        },
    ];
    let text = set_values(Some("extra = 1\n"), &edits).unwrap();
    let values = layer_values(Some(&text)).unwrap();
    assert_eq!(values["video.shader"], Value::from("crt-geom"));
    assert!(text.contains("extra = 1"), "unknown keys survive: {text}");

    let removed = set_values(
        Some(&text),
        &[KeyEdit {
            key: "video.shader".into(),
            value: None,
        }],
    )
    .unwrap();
    assert!(!removed.contains("shader"), "{removed}");

    let bad = [KeyEdit {
        key: "input_keyboard.b".into(),
        value: Some(serde_json::json!(["Escape"])),
    }];
    assert!(set_values(Some(&text), &bad).is_err(), "reserved key");
    assert!(layer_values(Some("not toml [")).is_err());
    assert!(layer_values(None).unwrap().is_empty());
}
