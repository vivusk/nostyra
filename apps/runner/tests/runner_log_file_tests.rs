//! Runner log files (`<data>/logs/runner-YYYYMMDD-HHMMSS-<system>.log`): a game
//! session writes one (also to stderr, for the launcher's error details) and never
//! the RetroAchievements token, even at debug level with a logged-in fake server;
//! a `--probe-core` service call writes none.
//!
//! Skips with a note when the mesen core is missing (`scripts/fetch-mesen-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use support::{output_within, registry_core, repo, scratch};

const TOKEN: &str = "tok-secret-4711";

fn runner_logs(data: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(data.join("logs")) else {
        return Vec::new();
    };
    entries.map(|entry| entry.unwrap().path()).collect()
}

#[test]
fn session_writes_one_log_file_without_the_token() {
    let Some(core) = registry_core("mesen", "fetch-mesen-core") else {
        return;
    };
    let data = scratch("log-file-session");
    let fake = data.join("fake-server");
    std::fs::create_dir_all(&fake).unwrap();
    let set = achievements::fake_game_set_json(77, "NES Test", &[(1, "0xH0001=11")], &[]);
    std::fs::write(fake.join("achievementsets.json"), set).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"));
    command
        .arg("--core")
        .arg(&core)
        .arg("--rom")
        .arg(repo("test-assets/roms/nostyra-test.nes"))
        .arg("--data-dir")
        .arg(&data)
        .args(["--system", "nes", "--headless-frames", "120", "--ipc"])
        .env("NOSTYRA_LOG", "debug")
        .env("NOSTYRA_RA_USER", "Tester")
        .env("NOSTYRA_RA_TOKEN", TOKEN)
        .env("NOSTYRA_RA_FAKE_SERVER", &fake);
    let out = output_within(command, Duration::from_secs(60), "logged session");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert!(
        stderr.contains("game session starting"),
        "stderr still logs"
    );

    let logs = runner_logs(&data);
    assert_eq!(logs.len(), 1, "{logs:?}");
    let name = logs[0].file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with("runner-") && name.ends_with("-nes.log"),
        "{name}"
    );
    let text = std::fs::read_to_string(&logs[0]).unwrap();
    assert!(text.contains("game session starting"), "{text}");
    assert!(text.contains("DEBUG"), "NOSTYRA_LOG=debug applies: {text}");
    assert!(!text.contains('\u{1b}'), "no ANSI colours in the file");
    assert!(!text.contains(TOKEN), "token leaked into the log: {text}");
    assert!(!stderr.contains(TOKEN), "token leaked into stderr");
}

#[test]
fn probe_core_writes_no_log_file() {
    let Some(core) = registry_core("mesen", "fetch-mesen-core") else {
        return;
    };
    let data = scratch("log-file-probe");
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"));
    command
        .arg("--probe-core")
        .arg(&core)
        .arg("--data-dir")
        .arg(&data);
    let out = output_within(command, Duration::from_secs(30), "probe");
    assert_eq!(out.status.code(), Some(0));
    assert!(runner_logs(&data).is_empty());
}
