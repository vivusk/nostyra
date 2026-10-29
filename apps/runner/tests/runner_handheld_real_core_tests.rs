//! Game Boy, Game Boy Color and Game Boy Advance end to end through the runner binary
//! on mGBA with the self-written test ROMs (`--system gb|gbc|gba` as the launcher
//! passes it): battery SRAM persists across runs (boot counter) under
//! `saves/<system>/`, the `.srm` keeps its size and no `.srm.bak` appears (mGBA
//! reports a 128 KiB GBA save buffer until the game first touches it), the Game Boy
//! MBC3 clock round-trips through `rtc.bin`, and a slot saved headless loads back
//! before frame 1.
//!
//! Skips with a note when the mgba core is missing (`scripts/fetch-mgba-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use support::{repo, result_line, runner, scratch};

const SYSTEMS: [&str; 3] = ["gb", "gbc", "gba"];

fn mgba() -> Option<PathBuf> {
    let core = system_catalog::core("mgba").unwrap();
    let path = repo(&format!(
        "cores/{}.{}",
        core.library_stem,
        std::env::consts::DLL_EXTENSION
    ));
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skipped: run scripts/fetch-mgba-core first");
        None
    }
}

fn rom(system: &str) -> PathBuf {
    repo(&format!("test-assets/roms/nostyra-test.{system}"))
}

fn saves(data: &Path, system: &str) -> PathBuf {
    support::game_saves_dir(data, system, &rom(system))
}

fn run_ok(core: &Path, data: &Path, system: &str, extra: &[&str]) -> String {
    let mut args = vec!["--system", system];
    args.extend_from_slice(extra);
    let out = runner(core, &rom(system), data, &args);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{system}: {stderr}");
    result_line(&out)
}

#[test]
fn handheld_sram_persists_with_a_stable_size() {
    let Some(core) = mgba() else { return };
    for system in SYSTEMS {
        let data = scratch(&format!("handheld-sram-{system}"));
        let dir = saves(&data, system);
        let srm = dir.join(ipc_protocol::SRAM_FILE_NAME);
        run_ok(&core, &data, system, &["--headless-frames", "30"]);
        let first = std::fs::read(&srm).unwrap();
        assert_eq!(&first[..2], &[1, 0xA5], "{system}: boot counter 1 + marker");
        run_ok(&core, &data, system, &["--headless-frames", "30"]);
        let second = std::fs::read(&srm).unwrap();
        assert_eq!(second[0], 2, "{system}: counter survived the restart");
        assert_eq!(second.len(), first.len(), "{system}: .srm size stable");
        let mut backup = srm.clone().into_os_string();
        backup.push(".bak");
        assert!(!Path::new(&backup).exists(), "{system}: no .srm.bak");
        let rtc = dir.join(ipc_protocol::RTC_FILE_NAME).exists();
        assert_eq!(
            rtc,
            system == "gb",
            "{system}: rtc.bin only for the MBC3 cart"
        );
    }
}

/// mGBA's MBC3 clock block: 5 live + 5 latched registers (u32 LE), then the Unix time
/// of the last latch (u64 LE). The ROM latches every frame, so after a run the live
/// registers hold the clock written into `rtc.bin` plus the wall time since its stamp.
#[test]
fn gb_clock_keeps_running_across_restarts() {
    let Some(core) = mgba() else { return };
    let data = scratch("handheld-rtc");
    let path = saves(&data, "gb").join(ipc_protocol::RTC_FILE_NAME);
    run_ok(&core, &data, "gb", &["--headless-frames", "60"]);
    assert_eq!(std::fs::read(&path).unwrap().len(), 48, "rtc.bin written");

    // Day 0, 00:00:00 one hour ago: the next run must show about 01:00:00 on day 0.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut clock = vec![0u8; 40];
    clock.extend_from_slice(&(now - 3600).to_le_bytes());
    std::fs::write(&path, &clock).unwrap();
    run_ok(&core, &data, "gb", &["--headless-frames", "60"]);
    let saved = std::fs::read(&path).unwrap();
    let reg = |i: usize| u32::from_le_bytes(saved[i * 4..i * 4 + 4].try_into().unwrap());
    let seconds = reg(0) + 60 * reg(1) + 3600 * reg(2);
    assert_eq!((reg(3), reg(4) & 1), (0, 0), "still day 0: {saved:02x?}");
    assert!(
        (3600..3720).contains(&seconds),
        "clock advanced by the hour since the stamp: {seconds} s"
    );
}

#[test]
fn handheld_saved_slot_loads_before_first_frame() {
    let Some(core) = mgba() else { return };
    let checksum = |line: &str| {
        line.split(' ')
            .find(|f| f.starts_with("checksum="))
            .unwrap()
            .to_string()
    };
    for system in SYSTEMS {
        let data = scratch(&format!("handheld-slot-{system}"));
        let saved = run_ok(
            &core,
            &data,
            system,
            &["--headless-frames", "90", "--headless-save-slot", "1"],
        );
        assert!(saved.ends_with("loaded=false"), "{system}: {saved}");
        let slot = saves(&data, system).join("slot1.state");
        assert!(slot.metadata().unwrap().len() > 0, "{system}: slot written");
        let load = ["--headless-frames", "5", "--load-state", "1"];
        let warm1 = run_ok(&core, &data, system, &load);
        let warm2 = run_ok(&core, &data, system, &load);
        assert!(warm1.ends_with("loaded=true"), "{system}: {warm1}");
        assert_eq!(checksum(&warm1), checksum(&warm2), "{system}: same frames");
        let cold = run_ok(&core, &data, system, &["--headless-frames", "5"]);
        assert_ne!(checksum(&cold), checksum(&warm1), "{system}: slot loaded");
    }
}
