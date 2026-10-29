//! GameCube on Dolphin through the runner binary with the committed test DOL
//! (`test-assets/roms/nostyra-test.dol`): 20 IPC sessions in a row play and quit with
//! no teardown-watchdog warning (Null renderer, every OS); a run writes only the
//! core's `User/` tree inside the game's save folder; startup resume of `auto` right
//! after the load works twice in a row (#322, OpenGL renderer, `gl_test_gate`); a
//! Null-renderer IPC session with a resume point ends cleanly and leaves it as it was.
//! Covered elsewhere: refused graphics request → exit 6 (`runner_dolphin_session_tests`),
//! forced slow teardown → watchdog exit + pending play-session file
//! (`runner_teardown_watchdog_tests`). Skipped without the core
//! (`scripts/fetch-dolphin-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ipc_protocol::{decode_event_line, RunnerEvent};
use support::{game_saves_dir, gl_test_gate, registry_core, repo, result_line, scratch};
use system_catalog::GC;

const FETCH: &str = "fetch-dolphin-core";
const DOL: &str = "test-assets/roms/nostyra-test.dol";
const NULL: [&str; 2] = ["--core-option", "dolphin_renderer=Null"];
/// The GL probe initialises SDL in this test process: one GL test at a time.
static GL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn dolphin() -> Option<PathBuf> {
    let core = system_catalog::core_for_system(GC.id).unwrap();
    registry_core(core.id, FETCH)
}

/// One runner process on the test DOL with `extra` args; panics unless it exits 0.
fn run_ok(core: &Path, data: &Path, extra: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .arg("--core")
        .arg(core)
        .arg("--rom")
        .arg(repo(DOL))
        .arg("--data-dir")
        .arg(data)
        .args(extra)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert!(!stderr.contains("core teardown timed out"), "{stderr}");
    out
}

fn events(out: &Output) -> Vec<RunnerEvent> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(decode_event_line)
        .collect()
}

/// Every file below `dir`, relative to it, with `/` separators.
fn files_below(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(dir).unwrap();
                found.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found
}

#[test]
fn twenty_ipc_sessions_play_and_quit_without_the_watchdog() {
    let Some(core) = dolphin() else { return };
    let data = scratch("gc-twenty-quits");
    let started = std::time::Instant::now();
    for round in 0..20 {
        let out = run_ok(
            &core,
            &data,
            &[&NULL[..], &["--ipc", "--headless-frames", "30"]].concat(),
        );
        let events = events(&out);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, RunnerEvent::Stopped { .. })),
            "round {round}: {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, RunnerEvent::Error { .. })),
            "{events:?}"
        );
    }
    eprintln!("20 Dolphin sessions in {:?}", started.elapsed());
}

/// Beside the runner's own files (system files, play-session records, the resume
/// point folder itself), the core writes only `User/**` in the game's save folder.
#[test]
fn a_run_writes_only_the_user_tree_in_the_game_folder() {
    let Some(core) = dolphin() else { return };
    let data = scratch("gc-user-tree");
    run_ok(
        &core,
        &data,
        &[&NULL[..], &["--ipc", "--headless-frames", "60"]].concat(),
    );
    let game = game_saves_dir(&data, GC.id, &repo(DOL));
    let game_rel = game
        .strip_prefix(&data)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let pending = ipc_protocol::pending_sessions_dir(&data);
    let pending_rel = pending
        .strip_prefix(&data)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let files = files_below(&data);
    let user = format!("{game_rel}/User/");
    assert!(
        files.iter().any(|f| f.starts_with(&user)),
        "core wrote User/: {files:?}"
    );
    let stray: Vec<&String> = files
        .iter()
        .filter(|f| !f.starts_with(&user))
        .filter(|f| !f.starts_with("system/dolphin-emu/"))
        .filter(|f| !f.starts_with(&format!("{pending_rel}/")))
        // The runner's own session log (`<data>/logs/`).
        .filter(|f| !f.starts_with("logs/"))
        .collect();
    assert!(stray.is_empty(), "files outside User/: {stray:?}");
    assert!(!data.join("cheats").exists(), "no <data>/cheats");
}

/// #322: a resume point loads right after the load, in two launches in a row.
#[test]
fn startup_resume_after_boot_works_twice_in_a_row() {
    let Some(core) = dolphin() else { return };
    let _gl = GL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !gl_test_gate() {
        return;
    }
    let data = scratch("gc-resume-twice");
    let save = ["--headless-frames", "120", "--headless-save-slot", "auto"];
    let saved = result_line(&run_ok(&core, &data, &save));
    assert!(saved.contains(" nonblack=true "), "{saved}");
    let auto = game_saves_dir(&data, GC.id, &repo(DOL)).join("auto.state");
    assert!(
        auto.metadata().unwrap().len() > 64 << 20,
        "~95 MB Dolphin state"
    );
    for round in 1..=2 {
        let resume = ["--headless-frames", "30", "--load-state", "auto"];
        let line = result_line(&run_ok(&core, &data, &resume));
        assert!(line.ends_with("loaded=true"), "round {round}: {line}");
        assert!(line.contains(" nonblack=true "), "round {round}: {line}");
    }
}

/// Null renderer + a resume point (as the launcher's Continue would pass it): the
/// session refuses it before the core, ends cleanly and never rewrites it.
#[test]
fn null_ipc_session_with_a_resume_point_ends_cleanly_and_keeps_it() {
    let Some(core) = dolphin() else { return };
    let data = scratch("gc-null-resume");
    let game = game_saves_dir(&data, GC.id, &repo(DOL));
    std::fs::create_dir_all(&game).unwrap();
    let sidecar = ipc_protocol::StateSidecar {
        core_name: "dolphin-emu".into(),
        ..ipc_protocol::StateSidecar::default()
    };
    let json = serde_json::to_vec(&sidecar).unwrap();
    std::fs::write(game.join("auto.state"), [9u8; 128]).unwrap();
    std::fs::write(game.join("auto.json"), &json).unwrap();
    let args = ["--ipc", "--headless-frames", "30", "--load-state", "auto"];
    let out = run_ok(&core, &data, &[&NULL[..], &args[..]].concat());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("save states need hardware rendering"),
        "{stderr}"
    );
    assert!(events(&out)
        .iter()
        .any(|e| matches!(e, RunnerEvent::Stopped { .. })));
    assert_eq!(std::fs::read(game.join("auto.state")).unwrap(), [9u8; 128]);
    assert_eq!(std::fs::read(game.join("auto.json")).unwrap(), json);
}
