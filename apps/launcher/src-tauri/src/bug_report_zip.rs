//! Bug-report zip (Settings → About → Create bug report, game error toast): the current
//! and previous launcher logs, the newest runner (game session) logs and
//! `system-info.txt`, written to `<data>/logs/nostyra-bug-report-YYYYMMDD-HHMMSS.zip`
//! for the user to attach to a GitHub issue. Logs are scrubbed of secrets first;
//! nothing is uploaded.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{Datelike, Timelike};
use ipc_protocol::log_files::{
    classify_log_entry, file_name_stamp, LogEntryKind, LogFileKind, BUG_REPORT_EXT,
    BUG_REPORT_PREFIX,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::log_secret_scrub::scrub_secrets;

/// Launcher logs in a report: this run's and the one before (a crash ends a run).
const LAUNCHER_LOGS_IN_REPORT: usize = 2;
/// Runner logs in a report: the newest game sessions.
const RUNNER_LOGS_IN_REPORT: usize = 5;
/// Name of the system facts entry inside the zip.
pub const SYSTEM_INFO_ENTRY: &str = "system-info.txt";

/// Writes a new bug-report zip into `logs_dir` and returns its path.
pub fn write_bug_report(logs_dir: &Path, system_info: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(logs_dir)?;
    let name = format!("{BUG_REPORT_PREFIX}{}{BUG_REPORT_EXT}", file_name_stamp());
    let path = logs_dir.join(name);
    let logs = select_report_logs(logs_dir);
    let written = write_zip(&path, system_info, &logs);
    if written.is_err() {
        let _ = fs::remove_file(&path); // no half-written zip left behind
    }
    written.map(|()| path)
}

/// The logs a report carries, newest first: launcher logs, then runner logs.
pub fn select_report_logs(logs_dir: &Path) -> Vec<PathBuf> {
    let mut launcher = Vec::new();
    let mut runner = Vec::new();
    for entry in fs::read_dir(logs_dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let modified = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        match classify_log_entry(&name) {
            Some(LogEntryKind::Log(LogFileKind::Launcher)) => {
                launcher.push((modified, name, entry.path()))
            }
            Some(LogEntryKind::Log(LogFileKind::Runner)) => {
                runner.push((modified, name, entry.path()))
            }
            _ => {}
        }
    }
    let newest = |mut files: Vec<(SystemTime, String, PathBuf)>, count: usize| {
        files.sort_by(|a, b| (b.0, &b.1).cmp(&(a.0, &a.1)));
        files.into_iter().take(count).map(|(_, _, path)| path)
    };
    newest(launcher, LAUNCHER_LOGS_IN_REPORT)
        .chain(newest(runner, RUNNER_LOGS_IN_REPORT))
        .collect()
}

fn write_zip(path: &Path, system_info: &str, logs: &[PathBuf]) -> io::Result<()> {
    let mut zip = ZipWriter::new(File::create(path)?);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(zip_now());
    zip.start_file(SYSTEM_INFO_ENTRY, options)
        .map_err(io::Error::other)?;
    zip.write_all(system_info.as_bytes())?;
    for log in logs {
        let Some(name) = log.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        // A log removed or unreadable since the listing is skipped, not fatal.
        let Ok(bytes) = fs::read(log) else { continue };
        let text = scrub_secrets(&String::from_utf8_lossy(&bytes));
        zip.start_file(name, options).map_err(io::Error::other)?;
        zip.write_all(text.as_bytes())?;
    }
    zip.finish().map_err(io::Error::other)?.sync_all()
}

/// Local time for the zip entries (the format's default is 1980-01-01).
fn zip_now() -> DateTime {
    let now = chrono::Local::now();
    let field = |value: u32| u8::try_from(value).unwrap_or_default();
    DateTime::from_date_and_time(
        u16::try_from(now.year()).unwrap_or(1980),
        field(now.month()),
        field(now.day()),
        field(now.hour()),
        field(now.minute()),
        field(now.second()),
    )
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "bug_report_zip_tests.rs"]
mod tests;
