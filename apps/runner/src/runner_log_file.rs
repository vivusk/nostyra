//! Runner logging: stderr (the launcher keeps the last lines for the error dialog's
//! Details) plus one log file per game session in `<data>/logs/`
//! (`runner-YYYYMMDD-HHMMSS-<system>.log`, see `ipc_protocol::log_files`). Quick
//! service calls (`--probe-core`, `--print-config`) log to stderr only.

use ipc_protocol::log_files::{init_logging, LogFileKind, LogFileRequest};

use crate::runner_cli_args::RunnerCliArgs;

/// Installs the process logger for this run; call once, right after parsing arguments.
pub fn start_runner_logging(args: &RunnerCliArgs) {
    let session = args.probe_core.is_none() && !args.print_config;
    // Same data root as the session itself (`RunnerDataPaths::resolve`).
    let data_root = session
        .then(|| args.data_dir.clone().or_else(ipc_protocol::data_root))
        .flatten();
    let request = data_root.as_deref().map(|data_root| LogFileRequest {
        data_root,
        kind: LogFileKind::Runner,
        label: args.system.as_deref(),
    });
    if let Some(path) = init_logging(request) {
        tracing::info!(log_file = %path.display(), "runner log file");
    }
    if session {
        tracing::info!(
            core = ?args.core,
            rom = ?args.rom,
            system = ?args.system,
            ipc = args.ipc,
            "game session starting"
        );
    }
}
