//! Hardcore through the runner binary (M15 phase 04): headless Mesen with
//! `nostyra-test.nes`, the debug fake server and `[achievements] hardcore = true` in
//! the global config. With the hidden flag forced on (`NOSTYRA_RA_HARDCORE_UI=1`):
//! state saving works, `--load-state` is skipped (game boots fresh, exit 0) and a
//! disallowed `--core-option` drops the session to softcore. Without the flag the
//! setting is ignored (softcore, the state loads).
//!
//! Skips with a note when the mesen core is missing (`scripts/fetch-mesen-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use ipc_protocol::{decode_event_line, RunnerEvent};
use support::{output_within, registry_core, repo, result_line, scratch};

const FLAG_ENV: &str = "NOSTYRA_RA_HARDCORE_UI";

/// One headless run: (session hardcore per `AchievementsLoaded`, result line).
fn run(core: &Path, data: &Path, flag: bool, extra: &[&str]) -> (Option<bool>, String) {
    let fake = data.join("fake-server");
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"));
    command
        .arg("--core")
        .arg(core)
        .arg("--rom")
        .arg(repo("test-assets/roms/nostyra-test.nes"))
        .arg("--data-dir")
        .arg(data)
        .args(["--system", "nes", "--headless-frames", "300", "--ipc"])
        .args(extra)
        .env("NOSTYRA_RA_USER", "Tester")
        .env("NOSTYRA_RA_TOKEN", "tok123")
        .env("NOSTYRA_RA_FAKE_SERVER", &fake)
        .env_remove(FLAG_ENV);
    if flag {
        command.env(FLAG_ENV, "1");
    }
    let out = output_within(command, Duration::from_secs(60), "hardcore run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    let hardcore = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(decode_event_line)
        .find_map(|event| match event {
            RunnerEvent::AchievementsLoaded { hardcore, .. } => Some(hardcore),
            _ => None,
        });
    (hardcore, result_line(&out))
}

#[test]
fn hardcore_refuses_state_loads_and_a_disallowed_option_drops_it() {
    let Some(core) = registry_core("mesen", "fetch-mesen-core") else {
        return;
    };
    let data = scratch("achievements-hardcore");
    std::fs::create_dir_all(data.join("fake-server")).unwrap();
    let set = achievements::fake_game_set_json(77, "NES Test", &[(1, "0xH0001=99")], &[]);
    std::fs::write(data.join("fake-server/achievementsets.json"), set).unwrap();
    std::fs::create_dir_all(data.join("config")).unwrap();
    std::fs::write(
        data.join("config/global.toml"),
        "[achievements]\nhardcore = true\n",
    )
    .unwrap();

    let (hardcore, _) = run(&core, &data, true, &["--headless-save-slot", "1"]);
    assert_eq!(hardcore, Some(true), "flag on: hardcore session");
    let (hardcore, line) = run(&core, &data, true, &["--load-state", "1"]);
    assert_eq!(hardcore, Some(true));
    assert!(
        line.ends_with("loaded=false"),
        "saved in hardcore, load refused: {line}"
    );

    let pal = ["--core-option", "mesen_region=PAL"];
    let (hardcore, _) = run(&core, &data, true, &pal);
    assert_eq!(hardcore, Some(false), "disallowed option → softcore");

    let (hardcore, line) = run(&core, &data, false, &["--load-state", "1"]);
    assert_eq!(hardcore, Some(false), "hidden flag: setting ignored");
    assert!(line.ends_with("loaded=true"), "{line}");
}
