//! Launcher-side runner management without Tauri: spawn a game session and follow
//! it through events and exit status, probe cores out of process, and import the
//! play time the runner records. The Tauri app only forwards [`SessionUpdate`]s.

#![deny(unsafe_code)]

mod pending_session_import;
mod runner_core_probe;
mod runner_exit_classification;
mod runner_launch;
mod runner_output_readers;
mod runner_process_command;
mod runner_session_manager;
mod session_update;

pub use pending_session_import::{
    import_pending_sessions, PendingImportError, PendingImportSummary,
};
pub use runner_core_probe::{probe_core_with_runner, ProbedCore};
pub use runner_exit_classification::{classify_exit, SessionErrorKind};
pub use runner_launch::{runner_args, RunnerLaunch, REDACTED_ENV};
pub use runner_session_manager::{spawn_runner_session, RunnerSession};
pub use session_update::{AchievementUpdate, SessionError, SessionOutcome, SessionUpdate};

/// No `Started` within this → the UI leaves its "Launching…" state.
pub const DEFAULT_LAUNCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
