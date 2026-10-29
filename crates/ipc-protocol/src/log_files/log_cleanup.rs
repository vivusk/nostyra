//! Automatic cleanup of the logs folder, run at every launcher and runner start so old
//! logs and bug reports never pile up. Only files named by Nostyra are touched; a file
//! another process still holds (Windows lock) is skipped silently.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::{classify_log_entry, LogEntryKind, LogFileKind};

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// How many logs to keep and for how long.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogCleanupPolicy {
    /// Newest launcher logs kept.
    pub keep_launcher_logs: usize,
    /// Newest runner (game session) logs kept.
    pub keep_runner_logs: usize,
    /// Any log last written longer ago is deleted.
    pub max_log_age: Duration,
    /// Any bug-report zip older than this is deleted.
    pub max_bug_report_age: Duration,
}

impl Default for LogCleanupPolicy {
    fn default() -> Self {
        Self {
            keep_launcher_logs: 10,
            keep_runner_logs: 30,
            max_log_age: 14 * DAY,
            max_bug_report_age: 7 * DAY,
        }
    }
}

struct Candidate {
    path: PathBuf,
    name: String,
    modified: SystemTime,
}

/// Deletes the logs and bug reports in `dir` that `policy` no longer keeps, never
/// `current` (this process's own log). Returns how many files were deleted; errors
/// (folder missing, file locked) are ignored.
pub fn clean_old_logs(
    dir: &Path,
    policy: &LogCleanupPolicy,
    now: SystemTime,
    current: Option<&Path>,
) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let (mut launcher, mut runner, mut reports) = (Vec::new(), Vec::new(), Vec::new());
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(kind) = classify_log_entry(&name) else {
            continue;
        };
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let candidate = Candidate {
            path: entry.path(),
            name,
            modified: meta.modified().unwrap_or(now),
        };
        match kind {
            LogEntryKind::Log(LogFileKind::Launcher) => launcher.push(candidate),
            LogEntryKind::Log(LogFileKind::Runner) => runner.push(candidate),
            LogEntryKind::BugReport => reports.push(candidate),
        }
    }
    let mut doomed = Vec::new();
    doomed.extend(over_limit(
        launcher,
        policy.keep_launcher_logs,
        policy.max_log_age,
        now,
    ));
    doomed.extend(over_limit(
        runner,
        policy.keep_runner_logs,
        policy.max_log_age,
        now,
    ));
    doomed.extend(over_limit(
        reports,
        usize::MAX,
        policy.max_bug_report_age,
        now,
    ));
    doomed
        .into_iter()
        .filter(|path| Some(path.as_path()) != current)
        .filter(|path| std::fs::remove_file(path).is_ok())
        .count()
}

/// Files beyond the newest `keep` (by write time, then name) or older than `max_age`.
fn over_limit(
    mut files: Vec<Candidate>,
    keep: usize,
    max_age: Duration,
    now: SystemTime,
) -> Vec<PathBuf> {
    files.sort_by(|a, b| (b.modified, &b.name).cmp(&(a.modified, &a.name)));
    files
        .into_iter()
        .enumerate()
        .filter(|(index, file)| {
            let age = now.duration_since(file.modified).unwrap_or_default();
            *index >= keep || age > max_age
        })
        .map(|(_, file)| file.path)
        .collect()
}

#[cfg(test)]
#[path = "log_cleanup_tests.rs"]
mod tests;
