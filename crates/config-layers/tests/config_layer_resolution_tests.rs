//! Layer resolution and writing against real files in a scratch data root.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};

use config_layers::{
    resolve_config, set_layer_value, ConfigError, ConfigLayerPaths, ResolvedConfig, Value,
    VideoScaling,
};

/// Fresh scratch data root per test.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("config-{name}"));
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
fn missing_files_give_defaults() {
    let root = scratch("missing");
    let config = resolve_config(&layers(&root), None);
    // Built-in defaults share the triggers (L2/R2 vs hold hotkeys): resolved here.
    let mut expected = ResolvedConfig::default();
    config_layers::resolve_input_conflicts(&mut expected.input, None);
    assert_eq!(config, expected);
    assert_eq!(config.video.shader, "none");
    assert_eq!(config.rewind.buffer_mb, 64);
    assert!(!config.rewind.enabled && config.latency.run_ahead_frames == 0);
}

#[test]
fn game_beats_core_beats_system_beats_global() {
    let root = scratch("order");
    let paths = layers(&root);
    write(&paths.global, "[video]\nshader = \"crt-easymode\"\nscaling = \"integer\"\n[rewind]\nbuffer_mb = 128\n[latency]\nrun_ahead_frames = 1");
    write(
        &paths.system,
        "[video]\nshader = \"crt-geom\"\n[rewind]\nenabled = true",
    );
    write(
        &paths.core,
        "[video]\nshader = \"crt-lottes\"\n[rewind]\nbuffer_mb = 256",
    );
    write(&paths.game, "[video]\nshader = \"none\"");
    let config = resolve_config(&paths, None);
    assert_eq!(config.video.shader, "none"); // game
    assert_eq!(config.video.scaling, VideoScaling::Integer); // global (nested merge)
    assert_eq!(config.rewind.buffer_mb, 256); // core
    assert!(config.rewind.enabled); // system
    assert_eq!(config.latency.run_ahead_frames, 1); // global
}

#[test]
fn invalid_value_falls_back_per_key() {
    let root = scratch("invalid");
    let paths = layers(&root);
    write(
        &paths.global,
        "[video]\nshader = \"crt-geom\"\n[latency]\nrun_ahead_frames = 1",
    );
    write(
        &paths.game,
        "[video]\nshader = \"crt-royale\"\nscaling = \"stretch\"\n[latency]\nrun_ahead_frames = 9\n\
         [rewind]\nenabled = true\nbuffer_mb = 8",
    );
    let config = resolve_config(&paths, None);
    assert_eq!(config.video.shader, "crt-geom"); // invalid in game → global
    assert_eq!(config.video.scaling, VideoScaling::Stretch); // valid sibling kept
    assert_eq!(config.latency.run_ahead_frames, 1);
    assert!(config.rewind.enabled);
    assert_eq!(config.rewind.buffer_mb, 64); // invalid everywhere → default
}

#[test]
fn parse_error_skips_only_that_layer() {
    let root = scratch("parse");
    let paths = layers(&root);
    write(&paths.global, "[rewind]\nenabled = true");
    write(&paths.core, "[video\nshader = ");
    write(&paths.game, "video = 5\n[latency]\nrun_ahead_frames = 2");
    let config = resolve_config(&paths, None);
    assert!(config.rewind.enabled);
    assert_eq!(config.latency.run_ahead_frames, 2);
    assert_eq!(config.video.shader, "none"); // non-table section ignored
}

#[test]
fn writer_sets_overwrites_removes_and_prunes() {
    let root = scratch("writer");
    let game = layers(&root).game;
    write(
        &game,
        "# hand comment\ncustom = \"kept\"\n[video]\nfuture_key = 1",
    );
    set_layer_value(
        &game,
        "video.shader",
        Some(Value::String("crt-geom".into())),
    )
    .unwrap();
    set_layer_value(
        &game,
        "video.shader",
        Some(Value::String("crt-lottes".into())),
    )
    .unwrap();
    set_layer_value(&game, "rewind.enabled", Some(Value::Boolean(true))).unwrap();
    assert_eq!(
        resolve_config(&layers(&root), None).video.shader,
        "crt-lottes"
    );

    set_layer_value(&game, "rewind.enabled", None).unwrap();
    let text = std::fs::read_to_string(&game).unwrap();
    let table: toml::Table = toml::from_str(&text).unwrap();
    assert!(!table.contains_key("rewind"), "empty table pruned: {text}");
    assert_eq!(table["custom"].as_str(), Some("kept"));
    assert_eq!(table["video"]["future_key"].as_integer(), Some(1));
    assert!(!game.with_file_name("Game (USA).toml.tmp").exists());

    set_layer_value(&game, "video.shader", None).unwrap();
    assert_eq!(resolve_config(&layers(&root), None).video.shader, "none");
}

#[test]
fn writer_creates_file_and_rejects_bad_input() {
    let root = scratch("writer-errors");
    let global = layers(&root).global;
    set_layer_value(&global, "latency.run_ahead_frames", Some(Value::Integer(2))).unwrap();
    assert_eq!(
        resolve_config(&layers(&root), None)
            .latency
            .run_ahead_frames,
        2
    );

    let bad = set_layer_value(&global, "latency.run_ahead_frames", Some(Value::Integer(5)));
    assert!(matches!(bad, Err(ConfigError::InvalidValue { .. })));
    let empty_segment = set_layer_value(&global, "video..shader", None);
    assert!(matches!(empty_segment, Err(ConfigError::InvalidKey(_))));
    let through_scalar = set_layer_value(
        &global,
        "latency.run_ahead_frames.x",
        Some(Value::Integer(1)),
    );
    assert!(matches!(through_scalar, Err(ConfigError::InvalidKey(_))));

    write(&global, "not = [valid");
    let broken = set_layer_value(&global, "video.shader", Some(Value::String("none".into())));
    assert!(matches!(broken, Err(ConfigError::Parse { .. })));
    assert_eq!(std::fs::read_to_string(&global).unwrap(), "not = [valid");
}
