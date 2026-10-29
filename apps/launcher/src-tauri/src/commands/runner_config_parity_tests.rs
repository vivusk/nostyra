//! Launcher/runner config parity: for the same data dir and ROM, the launcher's
//! effective values (`get_gameplay_config` view: gameplay `resolved` + controls
//! `input.resolved`, what the Detail tab shows) equal what the real runner resolves
//! (`--print-config`), across all four layers, including `core/<core>.toml`
//! (registry core id vs the core's `library_name`), for a game of every core,
//! a per-game binding override
//! and a hand-made binding conflict (hotkey wins on both sides).
//!
//! Needs the built runner next to the test's `deps/` folder (`cargo test --workspace`
//! builds it; `NOSTYRA_RUNNER` overrides) and the cores from
//! `scripts/fetch-*-core`; skips with a note when missing, or when an OpenGL core
//! finds no OpenGL (runner exit 6). Cases for the other systems: `system_cases`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value as Json};

use super::GameDataTarget;
use crate::commands::gameplay_config_commands::build_view;
use crate::runner_binary_path::{RUNNER_FILE_NAME, RUNNER_PATH_ENV};

#[path = "runner_config_parity_system_tests.rs"]
mod system_cases;

#[path = "runner_config_parity_gc_wii_tests.rs"]
mod gc_wii_cases;

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(rel)
}

/// `target/<profile>/nostyra-runner[.exe]` (the test exe runs from `<profile>/deps`).
fn runner_exe() -> PathBuf {
    match std::env::var_os(RUNNER_PATH_ENV) {
        Some(path) if !path.is_empty() => path.into(),
        _ => {
            let exe = std::env::current_exe().unwrap();
            exe.parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(RUNNER_FILE_NAME)
        }
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Runner `--print-config` JSON flattened to the launcher's dotted keys; `None` (with
/// a skip note) when the core needs OpenGL and there is none here (exit 6).
fn runner_values(runner: &Path, core: &Path, rom: &Path, data: &Path) -> Option<Map<String, Json>> {
    let out = Command::new(runner)
        .args(["--print-config", "--core"])
        .arg(core)
        .arg("--rom")
        .arg(rom)
        .arg("--data-dir")
        .arg(data)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    if out.status.code() == Some(ipc_protocol::EXIT_GRAPHICS) {
        let reason = stderr.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("");
        eprintln!("skipped: the core needs OpenGL (runner exit 6: {reason})");
        return None;
    }
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The core's C stdio may print after the JSON: read one value only.
    let json = &stdout[stdout.find('{').expect("JSON on stdout")..];
    let nested: Map<String, Json> = serde_json::Deserializer::from_str(json)
        .into_iter()
        .next()
        .expect("config JSON")
        .unwrap();
    let mut flat = Map::new();
    for (section, keys) in nested {
        let Some(keys) = keys.as_object() else {
            flat.insert(section, keys); // top-level scalars (`system`)
            continue;
        };
        for (key, value) in keys {
            flat.insert(format!("{section}.{key}"), value.clone());
        }
    }
    Some(flat)
}

#[test]
fn launcher_resolution_matches_runner_print_config() {
    check_parity("snes", &repo("test-assets/roms/nostyra-test.sfc"));
}

fn check_parity(system: &str, rom: &Path) {
    let spec = system_catalog::core_for_system(system).unwrap();
    let (runner, core) = (
        runner_exe(),
        repo(&format!(
            "cores/{}.{}",
            spec.library_stem,
            std::env::consts::DLL_EXTENSION
        )),
    );
    if !runner.is_file() || !core.is_file() {
        eprintln!("skipped: needs {} and {}", runner.display(), core.display());
        return;
    }
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-")
        .tempdir()
        .unwrap();
    let data = temp.path();
    let rom = rom.to_path_buf();
    // The runner gets no --content-sha1 here: it hashes the ROM the same way.
    let spec_system = system_catalog::system(system).unwrap();
    let target = GameDataTarget {
        system: system.into(),
        rom_path: rom.clone(),
        content_sha1: game_library::game_content_sha1(&rom, spec_system).unwrap(),
        missing: false,
    };
    let layers = target.config_layers(data);
    // Global: F1 (save_state hotkey) also on game button `b` (every system has it)
    // → conflict, hotkey wins.
    write(
        &layers.global,
        "[video]\nshader = \"none\"\nscaling = \"integer\"\n[rewind]\nbuffer_mb = 128\n\
         [resume]\nmode = \"never\"\n[input_keyboard]\nb = [\"KeyZ\", \"F1\"]",
    );
    write(
        &layers.system,
        "[latency]\nrun_ahead_frames = 2\n[rewind]\nenabled = true",
    );
    write(&layers.core, "[latency]\nrun_ahead_frames = 1");
    // Per-game gamepad override on `a`, or on `y` for a pad without `a` (N64); the pad
    // save_state hotkey on `misc1`, free on every pad (the stick clicks are L3 / R3).
    let actions = target.system_actions().unwrap();
    let overridden = if actions.contains(&"a") { "a" } else { "y" };
    write(
        &layers.game,
        &format!(
            "[video]\nshader = \"crt-geom\"\n[extra]\nkept = 1\n[resume]\nauto_save = false\n\
             [input_gamepad]\n{overridden} = [\"north\"]\n\
             [hotkeys_gamepad]\nsave_state = [\"misc1\"]"
        ),
    );

    let view = build_view(&layers.global, Some(&layers), target.system_actions());
    assert_eq!(view.input.warnings.len(), 1, "{:?}", view.input.warnings);
    let mut values = view.resolved;
    values.extend(view.input.resolved);
    let launcher = serde_json::to_value(values).unwrap();
    let Some(mut runner) = runner_values(&runner, &core, &rom, data) else {
        return;
    };
    assert_eq!(
        runner.remove("system"),
        Some(Json::from(system)),
        "runner system"
    );
    let runner = Json::Object(runner);
    assert_eq!(launcher, runner, "launcher and runner resolve differently");
    // Each layer actually took part (core beats system, game beats global).
    assert_eq!(runner["latency.run_ahead_frames"], 1, "core layer picked");
    assert_eq!(runner["video.shader"], "crt-geom");
    assert_eq!(runner["video.scaling"], "integer");
    assert_eq!(runner["rewind.enabled"], true);
    assert_eq!(runner["rewind.buffer_mb"], 128);
    assert_eq!(runner["resume.mode"], "never");
    assert_eq!(runner["resume.auto_save"], false);
    assert_eq!(runner["input_keyboard.b"], serde_json::json!(["KeyZ"]));
    let key = format!("input_gamepad.{overridden}");
    assert_eq!(runner[key.as_str()], serde_json::json!(["north"]));
    assert_eq!(
        runner["input_gamepad.b"],
        serde_json::json!(["south"]),
        "inherited"
    );
    assert_eq!(
        runner["hotkeys_gamepad.save_state"],
        serde_json::json!(["misc1"])
    );
}
