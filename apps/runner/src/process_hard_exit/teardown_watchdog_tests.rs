//! Watchdog timing with recorded fire functions (no process exit), and the report a
//! fire makes through a real `SessionReporter` (pending play-session record).

use std::path::Path;
use std::sync::mpsc::{channel, Receiver};

use super::*;

const SHORT: Duration = Duration::from_millis(60);
/// Long enough for a fire that is due, short enough to keep the suite fast.
const WAIT: Duration = Duration::from_millis(1500);

fn recording(timeout: Duration) -> (TeardownWatchdog, Receiver<u8>) {
    let (sender, fired) = channel();
    let sender = Mutex::new(sender);
    let watchdog = TeardownWatchdog::new(timeout, move |code| {
        let _ = sender.lock().unwrap().send(code);
    });
    (watchdog, fired)
}

#[test]
fn slow_teardown_fires_with_the_guard_code() {
    let (watchdog, fired) = recording(SHORT);
    {
        let _slow = TestSleep(SHORT * 4); // declared first: drops after the guard armed
        let _guard = watchdog.guard(5).with_code(0);
    }
    assert_eq!(fired.recv_timeout(WAIT), Ok(0));
}

#[test]
fn fast_teardown_then_disarm_never_fires() {
    let (watchdog, fired) = recording(SHORT);
    drop(watchdog.guard(5));
    watchdog.disarm();
    assert!(fired.recv_timeout(SHORT * 4).is_err());
}

#[test]
fn never_armed_never_fires_and_arming_is_idempotent() {
    let (watchdog, fired) = recording(SHORT);
    assert!(fired.recv_timeout(SHORT * 2).is_err(), "not armed");
    watchdog.arm(3);
    watchdog.arm(4);
    drop(watchdog.guard(6));
    assert_eq!(fired.recv_timeout(WAIT), Ok(3));
    assert!(fired.recv_timeout(SHORT * 2).is_err(), "one fire only");
}

/// Stands in for a slow session drop.
struct TestSleep(Duration);

impl Drop for TestSleep {
    fn drop(&mut self) {
        std::thread::sleep(self.0);
    }
}

fn pending_records(dir: &Path) -> Vec<ipc_protocol::PendingSessionRecord> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
        .map(|bytes| serde_json::from_slice(&bytes).unwrap())
        .collect()
}

/// A watchdog firing through `reporter` like `reporting` does, minus the exit.
fn reporting_recorder(reporter: &SessionReporter) -> (TeardownWatchdog, Receiver<u8>) {
    let (sender, fired) = channel();
    let (sender, reporter) = (Mutex::new(sender), reporter.clone());
    let watchdog = TeardownWatchdog::new(SHORT * 3, move |code| {
        reporter.finished(code, Some(TEARDOWN_TIMED_OUT.to_owned()));
        let _ = sender.lock().unwrap().send(code);
    });
    (watchdog, fired)
}

#[test]
fn fire_reads_the_started_state_at_fire_time_and_reports_once() {
    let temp = tempfile::tempdir().unwrap();
    let pending = temp.path().join("pending");
    let reporter = SessionReporter::new(true, &temp.path().join("Game.iso"));
    let (watchdog, fired) = reporting_recorder(&reporter);
    watchdog.arm(0); // armed before the session started (early `?` return path)
    reporter.started("Dolphin", &pending);
    assert_eq!(fired.recv_timeout(WAIT), Ok(0));
    let records = pending_records(&pending);
    assert_eq!(records.len(), 1, "pending play-session record written");
    assert_eq!(records[0].exit_code, 0);
    reporter.finished(0, None); // main, too late: the watchdog already reported
    assert_eq!(pending_records(&pending).len(), 1);
}

#[test]
fn fire_before_the_session_started_writes_no_record() {
    let temp = tempfile::tempdir().unwrap();
    let reporter = SessionReporter::new(true, &temp.path().join("Game.iso"));
    let (watchdog, fired) = reporting_recorder(&reporter);
    watchdog.arm(5);
    assert_eq!(fired.recv_timeout(WAIT), Ok(5));
    reporter.started("Dolphin", &temp.path().join("pending"));
    assert!(pending_records(&temp.path().join("pending")).is_empty());
}
