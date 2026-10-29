//! RetroAchievements through the runner binary (M15 phase 03): headless Mesen with
//! `nostyra-test.nes` against the debug fake server (`NOSTYRA_RA_FAKE_SERVER`, real
//! HTTP worker): the set loads before the unlock, the unlock reaches the launcher
//! (`--ipc`), the snapshot is written and the journal entry is gone. Logged out →
//! no achievement traffic or files at all.
//!
//! Skips with a note when the mesen core is missing (`scripts/fetch-mesen-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use ipc_protocol::{decode_event_line, RunnerEvent};
use support::{output_within, registry_core, repo, scratch};

/// The test ROM's hue counter ($01) steps 0..=11 once per frame.
const HUE_IS_11: &str = "0xH0001=11";

fn run(core: &Path, data: &Path, env: &[(&str, &Path)]) -> Vec<RunnerEvent> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"));
    command
        .arg("--core")
        .arg(core)
        .arg("--rom")
        .arg(repo("test-assets/roms/nostyra-test.nes"))
        .arg("--data-dir")
        .arg(data)
        .args(["--system", "nes", "--headless-frames", "300", "--ipc"])
        .env_remove("NOSTYRA_RA_USER")
        .env_remove("NOSTYRA_RA_TOKEN");
    for (key, value) in env {
        command.env(key, value);
    }
    let out = output_within(command, Duration::from_secs(60), "achievement run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(decode_event_line)
        .collect()
}

#[test]
fn fake_server_unlock_reaches_the_launcher_after_the_set_loaded() {
    let Some(core) = registry_core("mesen", "fetch-mesen-core") else {
        return;
    };
    let data = scratch("achievements-fake-server");
    let fake = data.join("fake-server");
    std::fs::create_dir_all(&fake).unwrap();
    let set = achievements::fake_game_set_json(77, "NES Test", &[(1, HUE_IS_11)], &[]);
    std::fs::write(fake.join("achievementsets.json"), set).unwrap();
    let env = [
        ("NOSTYRA_RA_USER", Path::new("Tester")),
        ("NOSTYRA_RA_TOKEN", Path::new("tok123")),
        ("NOSTYRA_RA_FAKE_SERVER", fake.as_path()),
    ];
    let events = run(&core, &data, &env);
    let loaded = events.iter().position(|e| {
        matches!(
            e,
            RunnerEvent::AchievementsLoaded {
                ra_game_id: 77,
                total: 1,
                ..
            }
        )
    });
    let unlocked = events.iter().position(|e| {
        *e == RunnerEvent::AchievementUnlocked {
            id: 1,
            hardcore: false,
        }
    });
    assert!(loaded.is_some() && unlocked.is_some(), "{events:?}");
    assert!(
        loaded < unlocked,
        "set loaded before the unlock: {events:?}"
    );
    let games = data.join("achievements").join("games");
    let snapshot = std::fs::read_dir(&games)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(snapshot).unwrap();
    assert!(
        text.contains("\"NES Test\"") && text.contains("\"unlocked\": true"),
        "{text}"
    );
    let pending = achievements::PendingJournal::new(&data);
    assert!(
        pending.list().is_empty(),
        "delivered unlock removed from the journal"
    );
}

#[test]
fn logged_out_run_has_no_achievement_traffic_or_files() {
    let Some(core) = registry_core("mesen", "fetch-mesen-core") else {
        return;
    };
    let data = scratch("achievements-logged-out");
    let events = run(&core, &data, &[]);
    assert!(
        events
            .iter()
            .all(|e| matches!(e, RunnerEvent::Started { .. } | RunnerEvent::Stopped { .. })),
        "{events:?}"
    );
    assert!(!data.join("achievements").exists());
}
