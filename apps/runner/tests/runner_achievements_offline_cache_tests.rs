//! Offline achievement sessions through the runner binary (M15 follow-up; debug
//! `NOSTYRA_RA_FAKE_SERVER` status files, real HTTP worker, temp data root): online
//! run (answers cached; the award is refused so the cached session has no unlock) →
//! offline run (set from the cache, unlock journaled) → online run (journal replayed
//! and removed; the server reports the unlock) → offline run (not unlocked again).
//!
//! Skips with a note when the mesen core is missing (`scripts/fetch-mesen-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use achievements::PendingJournal;
use ipc_protocol::{decode_event_line, RunnerEvent};
use support::{output_within, registry_core, repo, scratch};

const GAME_ID: u32 = 9007; // test-assets/achievements/nes/case.json
const ACHIEVEMENT_ID: u32 = 90071;
const OFFLINE: &[(&str, &str)] = &[
    ("login2.status", "-2"),
    ("achievementsets.status", "-2"),
    ("startsession.status", "-2"),
    ("awardachievement.status", "-2"),
    ("ping.status", "-2"),
];

/// A fresh `data/fake-server`: the canned NES set plus `overrides` (file, contents).
fn fake_server(data: &Path, overrides: &[(&str, &str)]) -> PathBuf {
    let dir = data.join("fake-server");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    let canned = repo("test-assets/achievements/nes");
    for name in ["achievementsets.json", "game-hash.txt"] {
        std::fs::copy(canned.join(name), dir.join(name)).unwrap();
    }
    for (name, contents) in overrides {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    dir
}

/// One headless NES run against `fake`; IPC events (exit code 0 asserted).
fn run(core: &Path, data: &Path, fake: &Path) -> Vec<RunnerEvent> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"));
    command
        .arg("--core")
        .arg(core)
        .arg("--rom")
        .arg(repo("test-assets/roms/nostyra-test.nes"))
        .arg("--data-dir")
        .arg(data)
        .args(["--system", "nes", "--headless-frames", "600", "--ipc"])
        .env("NOSTYRA_OFFLINE", "1")
        .env("NOSTYRA_RA_USER", "Tester")
        .env("NOSTYRA_RA_TOKEN", "tok123")
        .env("NOSTYRA_RA_FAKE_SERVER", fake)
        .env_remove("NOSTYRA_RA_HARDCORE_UI");
    let out = output_within(command, Duration::from_secs(90), "offline cache run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(decode_event_line)
        .collect()
}

fn loaded(events: &[RunnerEvent]) -> bool {
    events.iter().any(|e| {
        matches!(e, RunnerEvent::AchievementsLoaded { ra_game_id, hardcore: false, .. }
            if *ra_game_id == GAME_ID)
    })
}

fn unlocked(events: &[RunnerEvent]) -> bool {
    events.iter().any(|e| {
        *e == RunnerEvent::AchievementUnlocked {
            id: ACHIEVEMENT_ID,
            hardcore: false,
        }
    })
}

#[test]
fn offline_runs_use_cached_answers_and_the_journal_syncs_when_back_online() {
    let Some(core) = registry_core("mesen", "fetch-mesen-core") else {
        return;
    };
    let data = scratch("achievements-offline-cache");
    let journal = PendingJournal::new(&data);
    let refusal = r#"{"Success":false,"Error":"Achievement not found"}"#;
    let online = fake_server(&data, &[("awardachievement.json", refusal)]);
    let events = run(&core, &data, &online);
    assert!(loaded(&events) && unlocked(&events), "{events:?}");
    assert!(journal.list().is_empty(), "refused award dropped");
    let hash = std::fs::read_to_string(online.join("game-hash.txt")).unwrap();
    let user_cache = data.join("achievements/offline-cache/tester");
    let session_file = format!("sessions/{}.json", hash.trim().to_ascii_lowercase());
    assert!(user_cache.join("login2.json").exists());
    assert!(user_cache.join(&session_file).exists(), "{session_file}");

    let events = run(&core, &data, &fake_server(&data, OFFLINE));
    assert!(loaded(&events), "set from the cache: {events:?}");
    assert!(unlocked(&events), "unlocked offline: {events:?}");
    let pending = journal.list();
    assert_eq!(pending.len(), 1, "offline unlock journaled");
    assert_eq!(pending[0].1.id, ACHIEVEMENT_ID);

    let session = format!(
        "{{\"Success\":true,\"Unlocks\":[{{\"ID\":{ACHIEVEMENT_ID},\"When\":1700000000}}],\
         \"HardcoreUnlocks\":[],\"ServerNow\":1700000000}}"
    );
    let back = fake_server(&data, &[("startsession.json", &session)]);
    let events = run(&core, &data, &back);
    assert!(loaded(&events) && !unlocked(&events), "{events:?}");
    assert!(journal.list().is_empty(), "journal replayed + removed");

    let events = run(&core, &data, &fake_server(&data, OFFLINE));
    assert!(loaded(&events), "{events:?}");
    assert!(
        !unlocked(&events),
        "synced unlock not shown again: {events:?}"
    );
    assert!(journal.list().is_empty());
}
