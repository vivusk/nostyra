//! `--ipc` and `--probe-core` contract with the launcher: stdout events, pending
//! play-session file, closed-stdout safety.
//!
//! Requires the snes9x core fetched by `scripts/fetch-snes9x-core.{sh,ps1}`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use ipc_protocol::{decode_event_line, PendingSessionRecord, RunnerEvent};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn core() -> PathBuf {
    let path = repo(&format!(
        "cores/snes9x_libretro.{}",
        std::env::consts::DLL_EXTENSION
    ));
    assert!(
        path.exists(),
        "missing {}; run scripts/fetch-snes9x-core first",
        path.display()
    );
    path
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("runner-ipc-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn runner() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
}

fn ipc_run(rom: &Path, data: &Path) -> Output {
    runner()
        .arg("--core")
        .arg(core())
        .arg("--rom")
        .arg(rom)
        .arg("--data-dir")
        .arg(data)
        .args(["--ipc", "--headless-frames", "30"])
        .output()
        .unwrap()
}

fn events(out: &Output) -> Vec<RunnerEvent> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(decode_event_line)
        .collect()
}

fn pending_files(data: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(ipc_protocol::pending_sessions_dir(data)) {
        Ok(entries) => entries.map(|e| e.unwrap().path()).collect(),
        Err(_) => Vec::new(),
    }
}

#[test]
fn ipc_session_emits_started_stopped_and_writes_pending_file() {
    let data = scratch("ok");
    let rom = repo("test-assets/roms/nostyra-test.sfc");
    let out = ipc_run(&rom, &data);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let events = events(&out);
    assert!(
        matches!(&events[..], [RunnerEvent::Started { core, .. }, RunnerEvent::Stopped { .. }] if core == "Snes9x"),
        "{events:?}"
    );
    // The headless result line is still printed for the existing smoke tests.
    assert!(String::from_utf8_lossy(&out.stdout).contains("frames=30 "));

    let files = pending_files(&data);
    assert_eq!(files.len(), 1, "{files:?}");
    let record: PendingSessionRecord =
        serde_json::from_slice(&std::fs::read(&files[0]).unwrap()).unwrap();
    assert_eq!(record.rom_path, rom.to_string_lossy());
    assert_eq!(record.exit_code, 0);
    assert!(record.started_at > 1_700_000_000);
}

#[test]
fn failed_start_reports_error_without_session() {
    let data = scratch("bad-rom");
    let tiny = data.join("tiny.sfc");
    std::fs::write(&tiny, [0u8; 100]).unwrap();
    let out = ipc_run(&tiny, &data);
    assert_eq!(out.status.code(), Some(4));
    assert!(
        matches!(&events(&out)[..], [RunnerEvent::Error { .. }]),
        "{:?}",
        events(&out)
    );
    assert!(pending_files(&data).is_empty());
}

#[test]
fn closed_stdout_does_not_change_the_exit_code() {
    let data = scratch("closed-pipe");
    let mut child = runner()
        .arg("--core")
        .arg(core())
        .arg("--rom")
        .arg(repo("test-assets/roms/nostyra-test.sfc"))
        .arg("--data-dir")
        .arg(&data)
        .args(["--ipc", "--headless-frames", "120"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdout.take()); // launcher went away before the runner wrote anything
    assert_eq!(child.wait().unwrap().code(), Some(0));
    assert_eq!(pending_files(&data).len(), 1, "play time still recorded");
}

#[test]
fn probe_core_reports_identity_or_exit_3() {
    let out = runner().arg("--probe-core").arg(core()).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(
        matches!(&events(&out)[..], [RunnerEvent::CoreInfo { name, api_version: 1, .. }] if name == "Snes9x"),
        "{:?}",
        events(&out)
    );

    let fake = scratch("probe").join("fake-core.bin");
    std::fs::write(&fake, b"not a library").unwrap();
    let out = runner().arg("--probe-core").arg(&fake).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(matches!(&events(&out)[..], [RunnerEvent::Error { .. }]));
}
