//! Process-wide `tracing` setup shared by launcher and runner: stderr output (the
//! launcher shows the runner's last stderr lines as error details) plus, when a data
//! root is known, a size-capped log file in `<data>/logs/` with local timestamps.
//! Level: `NOSTYRA_LOG` (EnvFilter syntax), else `RUST_LOG`, else `info`.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use super::{
    clean_old_logs, create_log_file, logs_dir, LogCleanupPolicy, LogFileKind, SizeCappedWriter,
    LOG_SIZE_CAP_BYTES,
};

/// Environment variable with the log filter (checked before `RUST_LOG`).
pub const LOG_FILTER_ENV: &str = "NOSTYRA_LOG";

/// Which log file to start.
#[derive(Clone, Copy, Debug)]
pub struct LogFileRequest<'a> {
    pub data_root: &'a Path,
    pub kind: LogFileKind,
    /// Runner: the system id (part of the file name).
    pub label: Option<&'a str>,
}

/// Local wall-clock timestamps (`2026-10-29 15:40:02.123`): users compare them with
/// the time they saw the problem.
struct LocalClock;

impl FormatTime for LocalClock {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(
            w,
            "{}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
        )
    }
}

/// Installs the global subscriber (once per process; a second call is a no-op) and
/// returns the log file path. Without a file (no request, or the folder is not
/// writable) logging still goes to stderr. Old logs are cleaned up first.
pub fn init_logging(request: Option<LogFileRequest<'_>>) -> Option<PathBuf> {
    let opened = request.map(|request| open_log_file(&request));
    let (file_layer, path, file_error) = match opened {
        Some(Ok((file, path))) => {
            let writer = Mutex::new(SizeCappedWriter::new(file, LOG_SIZE_CAP_BYTES));
            let layer = fmt::layer()
                .with_ansi(false)
                .with_timer(LocalClock)
                .with_writer(writer);
            (Some(layer), Some(path), None)
        }
        Some(Err(err)) => (None, None, Some(err)),
        None => (None, None, None),
    };
    let console = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_timer(LocalClock);
    let installed = tracing_subscriber::registry()
        .with(env_filter())
        .with(console)
        .with(file_layer)
        .try_init()
        .is_ok();
    if installed {
        tracing::info!(
            version = env!("CARGO_PKG_VERSION"),
            os = std::env::consts::OS,
            arch = std::env::consts::ARCH,
            "Nostyra logging started"
        );
        if let Some(err) = file_error {
            tracing::warn!("cannot create the log file: {err}");
        }
    }
    path.filter(|_| installed)
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_env(LOG_FILTER_ENV)
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"))
}

/// Creates this process's log file, then trims the folder (never the new file).
fn open_log_file(request: &LogFileRequest<'_>) -> std::io::Result<(std::fs::File, PathBuf)> {
    let dir = logs_dir(request.data_root);
    let (file, path) = create_log_file(&dir, request.kind, request.label)?;
    let policy = LogCleanupPolicy::default();
    clean_old_logs(&dir, &policy, SystemTime::now(), Some(&path));
    Ok((file, path))
}
