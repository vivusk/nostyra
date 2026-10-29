//! `--print-config` end to end: config layers in a scratch data dir are resolved
//! (game over global) and CLI overrides win over every layer.
//!
//! Requires the snes9x core fetched by `scripts/fetch-snes9x-core.{sh,ps1}`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::Path;

use support::{core, runner, scratch, test_rom};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Runs `--print-config` and parses the JSON it prints (cores may add stdout noise
/// around it, so parse one value from the first `{`).
fn print_config(data: &Path, extra: &[&str]) -> serde_json::Value {
    let mut args = vec!["--print-config"];
    args.extend_from_slice(extra);
    let out = runner(&core(), &test_rom(), data, &args);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json = &stdout[stdout.find('{').expect("JSON on stdout")..];
    // The core's C stdio may flush its own output after ours: read one value only.
    serde_json::Deserializer::from_str(json)
        .into_iter()
        .next()
        .expect("config JSON")
        .unwrap()
}

#[test]
fn game_layer_beats_global_and_cli_beats_layers() {
    let data = scratch("print-config");
    let config = data.join("config");
    write(
        &config.join("global.toml"),
        "[video]\nshader = \"none\"\nscaling = \"integer\"\n[rewind]\nbuffer_mb = 128",
    );
    // Test ROM `nostyra-test.sfc` → legacy game file `nostyra-test.toml` (the runner
    // renames it to `nostyra-test [<id>].toml` and reads it); core "Snes9x" → `snes9x`.
    write(
        &config.join("core").join("snes9x.toml"),
        "[latency]\nrun_ahead_frames = 1",
    );
    write(
        &config.join("game").join("snes").join("nostyra-test.toml"),
        "[video]\nshader = \"crt-geom\"\n[rewind]\nenabled = true",
    );

    let resolved = print_config(&data, &[]);
    assert_eq!(resolved["video"]["shader"], "crt-geom");
    assert_eq!(resolved["video"]["scaling"], "integer");
    assert_eq!(resolved["latency"]["run_ahead_frames"], 1);
    assert_eq!(resolved["rewind"]["enabled"], true);
    assert_eq!(resolved["rewind"]["buffer_mb"], 128);

    let overridden = print_config(
        &data,
        &[
            "--run-ahead",
            "2",
            "--shader",
            "crt-lottes",
            "--rewind",
            "off",
        ],
    );
    assert_eq!(overridden["latency"]["run_ahead_frames"], 2);
    assert_eq!(overridden["video"]["shader"], "crt-lottes");
    assert_eq!(overridden["rewind"]["enabled"], false);
    assert_eq!(overridden["video"]["scaling"], "integer");
}

/// M3 exit criterion: a game-layer shader beats the global one, and removing the
/// key (as the launcher's "Use default" does) falls back to the global value.
#[test]
fn per_game_shader_override_and_fallback() {
    let data = scratch("print-config-override");
    let config = data.join("config");
    write(
        &config.join("global.toml"),
        "[video]
shader = \"none\"",
    );
    let game = support::game_config_path(&data, "snes", &test_rom());
    let name = game.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("nostyra-test [") && name.ends_with("].toml"),
        "{name}"
    );
    write(
        &game,
        "[video]
shader = \"crt-geom\"
scaling = \"stretch\"",
    );
    assert_eq!(print_config(&data, &[])["video"]["shader"], "crt-geom");

    config_layers::set_layer_value(&game, "video.shader", None).unwrap();
    let fallback = print_config(&data, &[]);
    assert_eq!(fallback["video"]["shader"], "none");
    assert_eq!(fallback["video"]["scaling"], "stretch", "other keys kept");
}

/// M4: `--print-config` shows the merged binding map: a game override of one
/// action leaves the rest global; a pad hotkey wins over a game action; a
/// hand-edited Escape / guide bind is dropped (both stay fixed menu inputs).
#[test]
fn input_sections_merge_per_action_and_reject_fixed_inputs() {
    let data = scratch("print-config-input");
    let config = data.join("config");
    write(
        &config.join("global.toml"),
        "[input_keyboard]\nb = [\"KeyO\"]\nstart = [\"Escape\"]\n\
         [hotkeys_gamepad]\npause_menu = [\"guide\"]",
    );
    write(
        &config.join("game").join("snes").join("nostyra-test.toml"),
        "[input_gamepad]\na = [\"south\"]\nr = [\"right_stick\"]\n\
         [hotkeys_gamepad]\nsave_state = [\"right_stick\"]",
    );
    let resolved = print_config(&data, &[]);
    assert_eq!(resolved["input_keyboard"]["b"], serde_json::json!(["KeyO"]));
    assert_eq!(resolved["input_keyboard"]["a"], serde_json::json!(["KeyX"]));
    assert_eq!(
        resolved["input_keyboard"]["start"],
        serde_json::json!(["Enter"]),
        "Escape rejected → default"
    );
    assert_eq!(resolved["input_gamepad"]["a"], serde_json::json!(["south"]));
    assert_eq!(resolved["input_gamepad"]["b"], serde_json::json!(["south"]));
    assert_eq!(
        resolved["input_gamepad"]["r"],
        serde_json::json!([]),
        "hotkey wins"
    );
    assert_eq!(
        resolved["hotkeys_gamepad"]["save_state"],
        serde_json::json!(["right_stick"])
    );
    assert_eq!(
        resolved["hotkeys_gamepad"]["pause_menu"],
        serde_json::json!([])
    );
}

#[test]
fn bad_override_is_a_usage_error() {
    let data = scratch("print-config-bad");
    let out = runner(
        &core(),
        &test_rom(),
        &data,
        &["--print-config", "--run-ahead", "3"],
    );
    assert_eq!(out.status.code(), Some(2));
}
