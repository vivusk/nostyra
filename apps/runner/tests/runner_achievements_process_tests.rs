//! RetroAchievements through the runner binary with the canned fake sets of
//! `test-assets/achievements/` (M15 phase 07; debug `NOSTYRA_RA_FAKE_SERVER`, real
//! HTTP worker, no pacing): the set (answered only for the ROM's hash) loads before the
//! unlock on stdout IPC for a cartridge (Mesen) and a disc (PCSX ReARMed); an offline
//! award stays in the journal after quit and the next run replays and removes it; a
//! permanent refusal drops a replayed entry; a rejected token ends the session cleanly.
//! The frame counters cycle (NES hue 0..11, 8-bit elsewhere), so an unlock follows
//! whenever the worker finishes the load.
//!
//! Each test skips with a note when its core is missing (`scripts/fetch-*-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use achievements::PendingJournal;
use ipc_protocol::{decode_event_line, RunnerEvent};
use support::{output_within, registry_core, repo, scratch};

/// `achievements` request answers written next to the canned set: (file, contents).
type Overrides<'a> = &'a [(&'a str, &'a str)];

/// `data/fake-server` = the canned set of `system` plus `overrides`.
fn fake_server(data: &Path, system: &str, overrides: Overrides<'_>) -> PathBuf {
    let dir = data.join("fake-server");
    std::fs::create_dir_all(&dir).unwrap();
    let canned = repo(&format!("test-assets/achievements/{system}"));
    for name in ["achievementsets.json", "game-hash.txt"] {
        std::fs::copy(canned.join(name), dir.join(name)).unwrap();
    }
    for (name, contents) in overrides {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    dir
}

/// One headless run of `rom` (`--system system`) against `fake`; IPC events + exit code 0.
fn run(core: &Path, (system, rom): (&str, &str), data: &Path, fake: &Path) -> Vec<RunnerEvent> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"));
    command
        .arg("--core")
        .arg(core)
        .arg("--rom")
        .arg(repo(&format!("test-assets/roms/{rom}")))
        .arg("--data-dir")
        .arg(data)
        .args(["--system", system, "--headless-frames", "600", "--ipc"])
        .env("NOSTYRA_OFFLINE", "1")
        .env("NOSTYRA_RA_USER", "Tester")
        .env("NOSTYRA_RA_TOKEN", "tok123")
        .env("NOSTYRA_RA_FAKE_SERVER", fake)
        .env_remove("NOSTYRA_RA_HARDCORE_UI");
    if system == "psx" {
        command.args(["--core-option", "pcsx_rearmed_bios=HLE"]); // no BIOS in CI
    }
    let out = output_within(command, Duration::from_secs(90), "achievement run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(decode_event_line)
        .collect()
}

fn game_id(system: &str) -> u32 {
    let case = repo(&format!("test-assets/achievements/{system}/case.json"));
    let case: serde_json::Value = serde_json::from_slice(&std::fs::read(case).unwrap()).unwrap();
    u32::try_from(case["game_id"].as_u64().unwrap()).unwrap()
}

fn position(events: &[RunnerEvent], wanted: impl Fn(&RunnerEvent) -> bool) -> Option<usize> {
    events.iter().position(wanted)
}

fn loaded_then_unlocked(system: &str, events: &[RunnerEvent]) {
    let game = game_id(system);
    let loaded = position(
        events,
        |e| matches!(e, RunnerEvent::AchievementsLoaded { ra_game_id, .. } if *ra_game_id == game),
    );
    let unlocked = position(events, |e| {
        matches!(e, RunnerEvent::AchievementUnlocked { .. })
    });
    assert!(
        loaded.is_some() && unlocked.is_some(),
        "{system}: {events:?}"
    );
    assert!(loaded < unlocked, "{system}: set loaded first: {events:?}");
}

fn skip_or_core(core_id: &str) -> Option<PathBuf> {
    registry_core(core_id, &format!("fetch-{core_id}-core"))
}

#[test]
fn canned_sets_load_before_the_unlock_for_a_cartridge_and_a_disc() {
    for (core_id, system, rom) in [
        ("mesen", "nes", "nostyra-test.nes"),
        ("pcsx-rearmed", "psx", "nostyra-test-psx.cue"),
    ] {
        let Some(core) = skip_or_core(core_id) else {
            continue;
        };
        let data = scratch(&format!("achievements-canned-{system}"));
        let fake = fake_server(&data, system, &[]);
        let events = run(&core, (system, rom), &data, &fake);
        loaded_then_unlocked(system, &events);
        assert!(PendingJournal::new(&data).list().is_empty(), "{system}");
    }
}

#[test]
fn offline_unlock_is_journaled_then_replayed_and_removed_next_run() {
    let Some(core) = skip_or_core("mesen") else {
        return;
    };
    let nes = ("nes", "nostyra-test.nes");
    let data = scratch("achievements-offline");
    let offline = fake_server(&data, "nes", &[("awardachievement.status", "-2")]);
    let events = run(&core, nes, &data, &offline);
    loaded_then_unlocked("nes", &events);
    let pending = PendingJournal::new(&data).list();
    assert_eq!(pending.len(), 1, "offline award kept after quit");
    assert_eq!(pending[0].1.id, game_id("nes") * 10 + 1);
    // Next run: the server is back but does not know the game (no new unlock), so only
    // the start-of-session replay can empty the journal.
    std::fs::remove_dir_all(&offline).unwrap();
    let back = fake_server(&data, "nes", &[("game-hash.txt", "0")]);
    let events = run(&core, nes, &data, &back);
    assert!(position(&events, |e| matches!(
        e,
        RunnerEvent::AchievementUnlocked { .. }
    ))
    .is_none());
    assert!(
        PendingJournal::new(&data).list().is_empty(),
        "replayed + removed"
    );
}

#[test]
fn a_permanently_refused_replay_is_dropped() {
    let Some(core) = skip_or_core("mesen") else {
        return;
    };
    let nes = ("nes", "nostyra-test.nes");
    let data = scratch("achievements-refused");
    let offline = fake_server(&data, "nes", &[("awardachievement.status", "-2")]);
    run(&core, nes, &data, &offline);
    assert_eq!(PendingJournal::new(&data).list().len(), 1);
    std::fs::remove_dir_all(&offline).unwrap();
    let refusal = r#"{"Success":false,"Error":"Achievement not found"}"#;
    let refused = [("game-hash.txt", "0"), ("awardachievement.json", refusal)];
    let back = fake_server(&data, "nes", &refused);
    run(&core, nes, &data, &back);
    assert!(PendingJournal::new(&data).list().is_empty(), "dropped");
}

#[test]
fn rejected_token_tells_the_launcher_and_exits_cleanly() {
    let Some(core) = skip_or_core("mesen") else {
        return;
    };
    let data = scratch("achievements-login-expired");
    let rejected = r#"{"Success":false,"Error":"Invalid token","Code":"invalid_credentials"}"#;
    let overrides = [("login2.json", rejected), ("login2.status", "401")];
    let fake = fake_server(&data, "nes", &overrides);
    let events = run(&core, ("nes", "nostyra-test.nes"), &data, &fake);
    assert!(
        events.contains(&RunnerEvent::AchievementsLoginExpired),
        "{events:?}"
    );
    let achievement = |e: &RunnerEvent| {
        matches!(
            e,
            RunnerEvent::AchievementsLoaded { .. } | RunnerEvent::AchievementUnlocked { .. }
        )
    };
    assert!(position(&events, achievement).is_none(), "{events:?}");
}
