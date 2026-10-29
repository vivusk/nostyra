use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::*;

const MINUTE: Duration = Duration::from_secs(60);

/// Writes `name` in `dir` with its write time `age` before `now`.
fn file_aged(dir: &Path, name: &str, now: SystemTime, age: Duration) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, b"line\n").unwrap();
    let file = File::options().write(true).open(&path).unwrap();
    file.set_modified(now - age).unwrap();
    path
}

#[test]
fn keeps_the_newest_launcher_and_runner_logs() {
    let dir = tempfile::tempdir().unwrap();
    let now = SystemTime::now();
    for i in 0..12u32 {
        file_aged(
            dir.path(),
            &format!("launcher-2026{i:02}.log"),
            now,
            MINUTE * i,
        );
    }
    for i in 0..33u32 {
        file_aged(
            dir.path(),
            &format!("runner-2026{i:02}-nes.log"),
            now,
            MINUTE * i,
        );
    }
    let deleted = clean_old_logs(dir.path(), &LogCleanupPolicy::default(), now, None);
    assert_eq!(deleted, 2 + 3);
    assert!(
        dir.path().join("launcher-202609.log").exists(),
        "10th newest kept"
    );
    assert!(!dir.path().join("launcher-202610.log").exists());
    assert!(!dir.path().join("launcher-202611.log").exists());
    assert!(dir.path().join("runner-202629-nes.log").exists());
    assert!(!dir.path().join("runner-202630-nes.log").exists());
}

#[test]
fn deletes_old_logs_and_old_bug_reports_only() {
    let dir = tempfile::tempdir().unwrap();
    let now = SystemTime::now();
    let day = Duration::from_secs(24 * 60 * 60);
    let old_log = file_aged(dir.path(), "runner-old-psx.log", now, day * 15);
    let fresh_log = file_aged(dir.path(), "runner-new-psx.log", now, day * 13);
    let old_zip = file_aged(dir.path(), "nostyra-bug-report-old.zip", now, day * 8);
    let fresh_zip = file_aged(dir.path(), "nostyra-bug-report-new.zip", now, day * 6);
    let foreign = file_aged(dir.path(), "my-notes.txt", now, day * 100);
    let deleted = clean_old_logs(dir.path(), &LogCleanupPolicy::default(), now, None);
    assert_eq!(deleted, 2);
    assert!(!old_log.exists() && !old_zip.exists());
    assert!(fresh_log.exists() && fresh_zip.exists());
    assert!(
        foreign.exists(),
        "files Nostyra did not name are left alone"
    );
}

#[test]
fn never_deletes_the_current_file() {
    let dir = tempfile::tempdir().unwrap();
    let now = SystemTime::now();
    let policy = LogCleanupPolicy {
        keep_launcher_logs: 1,
        ..LogCleanupPolicy::default()
    };
    file_aged(dir.path(), "launcher-b.log", now, MINUTE);
    let current = file_aged(dir.path(), "launcher-a.log", now, MINUTE * 2);
    assert_eq!(clean_old_logs(dir.path(), &policy, now, Some(&current)), 0);
    assert!(current.exists());
}

#[test]
fn missing_folder_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("logs");
    let policy = LogCleanupPolicy::default();
    assert_eq!(
        clean_old_logs(&missing, &policy, SystemTime::now(), None),
        0
    );
}
