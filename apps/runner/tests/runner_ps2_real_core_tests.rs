//! Sony PlayStation 2 on PCEE2 through the runner binary with the committed test disc
//! (`test-assets/roms/nostyra-test-ps2.iso`): zero BIOS → exit 4 with the "needs your
//! BIOS dump" text before any core loads (needs neither the DLL nor a BIOS: runs
//! everywhere); with the user's dump (`NOSTYRA_PS2_BIOS_DIR`, copied into the scratch
//! data folder, never into the repo) a launcher-style IPC session (`--system ps2`,
//! `--content-sha1`) plays and quits with exit 0 and a silent teardown watchdog, and
//! a refused hardware request (`NOSTYRA_FORCE_NO_GL=refuse`, debug runner) still
//! renders through the core's own readback device. The disc is copied into the
//! scratch folder: PCEE2 refuses a Windows path with `/` in it (M18 spike D7).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use ipc_protocol::{decode_event_line, RunnerEvent};
use support::{output_within, registry_core, repo, result_line, runner, scratch};
use system_catalog::{firmware_dir, parse_ps2_bios, PCEE2, PS2};

const FETCH: &str = "fetch-pcee2-core";
const BIOS_DIR_ENV: &str = "NOSTYRA_PS2_BIOS_DIR";
const ISO: &str = "test-assets/roms/nostyra-test-ps2.iso";
/// A debug runner boots the BIOS unpaced; generous so a slow machine never hangs the suite.
const DEADLINE: Duration = Duration::from_secs(240);

fn pcee2() -> Option<PathBuf> {
    let core = system_catalog::core_for_system(PS2.id).unwrap();
    assert_eq!(core.id, PCEE2.id);
    registry_core(core.id, FETCH)
}

/// The test disc copied under `data`, as a path in native separators throughout
/// (`CARGO_TARGET_TMPDIR` may carry a `/` from a relative `CARGO_TARGET_DIR`).
fn test_iso(data: &Path) -> PathBuf {
    let native: PathBuf = data.components().collect();
    let iso = native.join("nostyra-test-ps2.iso");
    std::fs::copy(repo(ISO), &iso).unwrap();
    iso
}

/// Copies the user's BIOS dump(s) into `<data>/system/pcsx2/bios/<stem>.bin`; false
/// (with a note) when `NOSTYRA_PS2_BIOS_DIR` is unset or holds no valid dump.
fn install_bios(data: &Path) -> bool {
    let Some(dir) = std::env::var_os(BIOS_DIR_ENV).filter(|d| !d.is_empty()) else {
        eprintln!("skipped: set {BIOS_DIR_ENV} to a folder with your PS2 BIOS");
        return false;
    };
    let target = firmware_dir(&data.join("system"), &PCEE2);
    std::fs::create_dir_all(&target).unwrap();
    let mut copied = 0;
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let bytes = std::fs::read(entry.path()).unwrap_or_default();
        if parse_ps2_bios(&bytes).is_some() {
            let stem = entry
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_lowercase();
            std::fs::write(target.join(stem).with_extension("bin"), bytes).unwrap();
            copied += 1;
        }
    }
    if copied == 0 {
        eprintln!("skipped: no valid PS2 BIOS in {BIOS_DIR_ENV}");
    }
    copied > 0
}

fn events(stdout: &[u8]) -> Vec<RunnerEvent> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(decode_event_line)
        .collect()
}

/// `prepare_core` refuses before `CoreSession::load`: the core path is never opened.
#[test]
fn zero_bios_is_refused_with_exit_4_before_any_core_loads() {
    let data = scratch("ps2-no-bios");
    let iso = test_iso(&data);
    let core = data.join("never-opened-core.dll");
    let out = runner(
        &core,
        &iso,
        &data,
        &["--system", PS2.id, "--headless-frames", "5"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_ROM_LOAD),
        "{stderr}"
    );
    assert!(
        stderr.contains("PlayStation 2 needs your BIOS dump"),
        "{stderr}"
    );
    assert!(
        !firmware_dir(&data.join("system"), &PCEE2)
            .parent()
            .unwrap()
            .join("memcards")
            .exists(),
        "the core never ran"
    );
}

/// The launcher's shape: `--ipc`, `--system ps2`, `--content-sha1` (the scan's id).
#[test]
fn ipc_session_with_the_content_id_plays_and_quits_without_the_watchdog() {
    let Some(core) = pcee2() else { return };
    let data = scratch("ps2-ipc");
    if !install_bios(&data) {
        return;
    }
    let iso = test_iso(&data);
    let sha1 = game_library::game_content_sha1(&iso, &PS2).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"));
    command.arg("--core").arg(&core).arg("--rom").arg(&iso);
    command.arg("--data-dir").arg(&data);
    command.args(["--ipc", "--system", PS2.id, "--content-sha1", &sha1]);
    command.args(["--headless-frames", "240"]);
    let out = output_within(command, DEADLINE, "PS2 IPC session");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert!(!stderr.contains("core teardown timed out"), "{stderr}");
    let events = events(&out.stdout);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, RunnerEvent::Stopped { .. })),
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, RunnerEvent::Error { .. })),
        "{events:?}"
    );
    let saves = ipc_protocol::saves_dir_for_game(&data, PS2.id, &iso, &sha1);
    assert!(saves.is_dir(), "game folder named by the content id");
    let memcards = firmware_dir(&data.join("system"), &PCEE2)
        .parent()
        .unwrap()
        .join("memcards");
    assert!(
        memcards.join("Mcd001.ps2").is_file(),
        "{}",
        memcards.display()
    );
}

/// `NOSTYRA_FORCE_NO_GL=refuse` (debug runner only) refuses the core's GL request:
/// PCEE2 falls back to its readback device (`hw_render_required: false`) and renders.
#[test]
fn refused_hardware_request_still_renders_through_readback() {
    if !cfg!(debug_assertions) {
        eprintln!("skipped: NOSTYRA_FORCE_NO_GL is read by debug runner builds only");
        return;
    }
    let Some(core) = pcee2() else { return };
    let data = scratch("ps2-refused-gl");
    if !install_bios(&data) {
        return;
    }
    let iso = test_iso(&data);
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"));
    command.env("NOSTYRA_FORCE_NO_GL", "refuse");
    command.arg("--core").arg(&core).arg("--rom").arg(&iso);
    command.arg("--data-dir").arg(&data);
    command.args(["--system", PS2.id, "--headless-frames", "300"]);
    let out = output_within(command, DEADLINE, "PS2 refused-GL run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    let line = result_line(&out);
    assert!(line.contains(" nonblack=true "), "{line}");
}
