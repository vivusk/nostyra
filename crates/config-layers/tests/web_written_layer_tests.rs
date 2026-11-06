//! A layer file written by the web edition (nostyra.com/online, phase 07 browser
//! smoke: Settings → Controls → A + M, copied byte for byte from the Data folder)
//! is read by the desktop like its own: same keys, same resolution.

#![cfg(feature = "fs")]
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};

use config_layers::{
    global_config_path, layer_known_values, resolve_config, set_layer_value, ConfigLayerPaths,
    Value,
};

const WEB_GLOBAL: &str = include_str!("fixtures/web-written-global.toml");

fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("web-written-layer");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn desktop_reads_and_extends_a_web_written_global_layer() {
    let root = scratch();
    let global = global_config_path(&root);
    std::fs::create_dir_all(global.parent().unwrap()).unwrap();
    std::fs::write(&global, WEB_GLOBAL).unwrap();
    let layers = ConfigLayerPaths::new(&root, "snes", "snes9x", Path::new("E:/ROMs/Game.sfc"), "");

    let config = resolve_config(&layers, None);
    assert_eq!(config.input.keyboard["a"], ["KeyX", "KeyM"]);
    assert_eq!(
        config.input.keyboard["b"],
        ["KeyZ"],
        "untouched keys keep the defaults"
    );

    // A desktop edit keeps the web's binding (and vice versa: same writer rules).
    set_layer_value(&global, "resume.mode", Some(Value::from("never"))).unwrap();
    let view = layer_known_values(&global).unwrap();
    assert_eq!(
        view["input_keyboard.a"],
        Value::Array(vec![Value::from("KeyX"), Value::from("KeyM")])
    );
    assert_eq!(view["resume.mode"], Value::from("never"));
}
