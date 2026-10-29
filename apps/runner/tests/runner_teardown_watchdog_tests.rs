//! Teardown watchdog through the runner binary (debug hooks): a teardown slower
//! than the timeout ends the process with the run's own exit code, after reporting
//! `Stopped` (+ the pending play-session record) and "core teardown timed out"; a
//! normal quit never fires it.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::process::{Command, Output};
use std::time::{Duration, Instant};

use ipc_protocol::{decode_event_line, RunnerEvent};
use support::{core, scratch, test_rom};

fn ipc_run(data: &std::path::Path, hooks: &[(&str, &str)]) -> (Output, Duration) {
    let started = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .envs(hooks.iter().copied())
        .arg("--core")
        .arg(core())
        .arg("--rom")
        .arg(test_rom())
        .arg("--data-dir")
        .arg(data)
        .args(["--ipc", "--headless-frames", "30"])
        .output()
        .unwrap();
    (out, started.elapsed())
}

fn events(out: &Output) -> Vec<RunnerEvent> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(decode_event_line)
        .collect()
}

fn pending_count(data: &std::path::Path) -> usize {
    std::fs::read_dir(ipc_protocol::pending_sessions_dir(data)).map_or(0, |dir| dir.count())
}

#[test]
fn hanging_teardown_is_cut_short_and_still_reported() {
    if !cfg!(debug_assertions) {
        eprintln!("skipped: the teardown hooks are read by debug runner builds only");
        return;
    }
    let data = scratch("teardown-hang");
    let hooks = [
        ("NOSTYRA_TEST_TEARDOWN_TIMEOUT_MS", "300"),
        ("NOSTYRA_TEST_TEARDOWN_DELAY_MS", "30000"),
    ];
    let (out, took) = ipc_run(&data, &hooks);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "the run's own code: {stderr}");
    assert!(took < Duration::from_secs(20), "watchdog fired ({took:?})");
    assert!(stderr.contains("core teardown timed out"), "{stderr}");
    let events = events(&out);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, RunnerEvent::Stopped { .. })),
        "{events:?}"
    );
    assert!(events.iter().any(
        |e| matches!(e, RunnerEvent::Error { message } if message == "core teardown timed out")
    ));
    assert_eq!(
        pending_count(&data),
        1,
        "pending play-session record written"
    );
}

#[test]
fn normal_quit_never_fires_the_watchdog() {
    let data = scratch("teardown-ok");
    let (out, _) = ipc_run(&data, &[("NOSTYRA_TEST_TEARDOWN_TIMEOUT_MS", "5000")]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert!(!stderr.contains("core teardown timed out"), "{stderr}");
    assert!(!events(&out)
        .iter()
        .any(|e| matches!(e, RunnerEvent::Error { .. })));
    assert_eq!(pending_count(&data), 1);
}
