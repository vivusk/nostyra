//! Log files under `<data>/logs/`: one file per launcher start
//! (`launcher-YYYYMMDD-HHMMSS.log`) and one per game session
//! (`runner-YYYYMMDD-HHMMSS-<system>.log`), plus the bug-report zips the launcher
//! creates there (`nostyra-bug-report-YYYYMMDD-HHMMSS.zip`). Names, cleanup policy,
//! the size cap and the `tracing` setup live here so launcher and runner agree.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

mod log_cleanup;
mod log_subscriber_setup;
mod size_capped_writer;

pub use log_cleanup::{clean_old_logs, LogCleanupPolicy};
pub use log_subscriber_setup::{init_logging, LogFileRequest};
pub use size_capped_writer::{SizeCappedWriter, LOG_SIZE_CAP_BYTES, LOG_TRUNCATED_NOTICE};

/// Folder name of the logs inside a data root.
pub const LOGS_DIR_NAME: &str = "logs";
/// File-name prefix of launcher logs.
pub const LAUNCHER_LOG_PREFIX: &str = "launcher-";
/// File-name prefix of runner (game session) logs.
pub const RUNNER_LOG_PREFIX: &str = "runner-";
/// File-name prefix of bug-report zips.
pub const BUG_REPORT_PREFIX: &str = "nostyra-bug-report-";
/// Extension of log files (with the dot).
pub const LOG_FILE_EXT: &str = ".log";
/// Extension of bug-report zips (with the dot).
pub const BUG_REPORT_EXT: &str = ".zip";

/// `<data>/logs/`.
pub fn logs_dir(data_root: &Path) -> PathBuf {
    data_root.join(LOGS_DIR_NAME)
}

/// Which process writes a log file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFileKind {
    Launcher,
    Runner,
}

impl LogFileKind {
    fn prefix(self) -> &'static str {
        match self {
            Self::Launcher => LAUNCHER_LOG_PREFIX,
            Self::Runner => RUNNER_LOG_PREFIX,
        }
    }
}

/// What a file in the logs folder is, judged by its name only (anything else in the
/// folder is left alone by cleanup and bug reports).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogEntryKind {
    Log(LogFileKind),
    BugReport,
}

/// Classifies a file name of the logs folder; `None` for files Nostyra did not write.
pub fn classify_log_entry(name: &str) -> Option<LogEntryKind> {
    if name.starts_with(LAUNCHER_LOG_PREFIX) && name.ends_with(LOG_FILE_EXT) {
        Some(LogEntryKind::Log(LogFileKind::Launcher))
    } else if name.starts_with(RUNNER_LOG_PREFIX) && name.ends_with(LOG_FILE_EXT) {
        Some(LogEntryKind::Log(LogFileKind::Runner))
    } else if name.starts_with(BUG_REPORT_PREFIX) && name.ends_with(BUG_REPORT_EXT) {
        Some(LogEntryKind::BugReport)
    } else {
        None
    }
}

/// `launcher-<stamp>.log` / `runner-<stamp>-<label>.log`; the label (a system id from
/// the command line) keeps only `[a-z0-9_-]` so it can never escape the folder.
pub fn log_file_name(kind: LogFileKind, stamp: &str, label: Option<&str>) -> String {
    let label: String = label
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(32)
        .collect::<String>()
        .to_ascii_lowercase();
    match label.is_empty() {
        true => format!("{}{stamp}{LOG_FILE_EXT}", kind.prefix()),
        false => format!("{}{stamp}-{label}{LOG_FILE_EXT}", kind.prefix()),
    }
}

/// Local time as used in log and bug-report names (`YYYYMMDD-HHMMSS`).
pub fn file_name_stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Creates a new log file in `dir` (never overwrites: a name taken by another process
/// started in the same second gets a `-<pid>` suffix).
pub fn create_log_file(
    dir: &Path,
    kind: LogFileKind,
    label: Option<&str>,
) -> io::Result<(File, PathBuf)> {
    std::fs::create_dir_all(dir)?;
    let stamp = file_name_stamp();
    let first = dir.join(log_file_name(kind, &stamp, label));
    match OpenOptions::new().write(true).create_new(true).open(&first) {
        Ok(file) => Ok((file, first)),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            let stamp = format!("{stamp}-{}", std::process::id());
            let second = dir.join(log_file_name(kind, &stamp, label));
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&second)?;
            Ok((file, second))
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_classify_and_labels_are_sanitised() {
        let name = log_file_name(LogFileKind::Runner, "20261029-154000", Some("../SNES x"));
        assert_eq!(name, "runner-20261029-154000-snesx.log");
        assert_eq!(
            log_file_name(LogFileKind::Launcher, "20261029-154000", None),
            "launcher-20261029-154000.log"
        );
        assert_eq!(
            classify_log_entry(&name),
            Some(LogEntryKind::Log(LogFileKind::Runner))
        );
        assert_eq!(
            classify_log_entry("nostyra-bug-report-20261029-154000.zip"),
            Some(LogEntryKind::BugReport)
        );
        assert_eq!(classify_log_entry("notes.txt"), None);
        assert_eq!(classify_log_entry("launcher-x.txt"), None);
    }

    #[test]
    fn same_second_logs_get_distinct_names() {
        let dir = tempfile::tempdir().unwrap();
        let (_a, first) = create_log_file(dir.path(), LogFileKind::Runner, Some("nes")).unwrap();
        let (_b, second) = create_log_file(dir.path(), LogFileKind::Runner, Some("nes")).unwrap();
        assert_ne!(first, second);
        assert!(first.is_file() && second.is_file());
    }
}
