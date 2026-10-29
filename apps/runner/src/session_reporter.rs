//! Reports a game session to the launcher (`--ipc`): `Started` once the game is
//! up, then on exit `Stopped` (after the final SRAM flush) and/or `Error`, plus a
//! pending play-session file so play time survives the launcher closing mid-game.
//! Without `--ipc` it does nothing.
//!
//! Cheap to clone and `Send`: the teardown watchdog (`process_hard_exit`) holds a
//! clone and reports from its own thread when a core never finishes unloading,
//! reading the started state at that moment. Only the first `finished` reports.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use config_layers::write_atomically;
use ipc_protocol::{pending_session_file_name, PendingSessionRecord, RunnerEvent};

use crate::ipc_event_writer::emit_event;

/// Set when the session has started; a session counts only after this.
struct StartedSession {
    at: Instant,
    unix_ms: u128,
    pending_dir: PathBuf,
}

#[derive(Clone)]
pub struct SessionReporter {
    shared: Arc<Shared>,
}

struct Shared {
    ipc: bool,
    rom: PathBuf,
    started: OnceLock<StartedSession>,
    /// Set by the first `finished` (main or the teardown watchdog).
    reported: AtomicBool,
}

impl SessionReporter {
    pub fn new(ipc: bool, rom: &Path) -> Self {
        let shared = Shared {
            ipc,
            rom: rom.to_path_buf(),
            started: OnceLock::new(),
            reported: AtomicBool::new(false),
        };
        Self {
            shared: Arc::new(shared),
        }
    }

    /// Core + game loaded (windowed: window open). Idempotent.
    pub fn started(&self, core_name: &str, pending_dir: &Path) {
        let shared = &*self.shared;
        if !shared.ipc || shared.started.get().is_some() {
            return;
        }
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis());
        let _ = shared.started.set(StartedSession {
            at: Instant::now(),
            unix_ms,
            pending_dir: pending_dir.to_path_buf(),
        });
        emit_event(&RunnerEvent::Started {
            core: core_name.to_string(),
            rom: shared.rom.to_string_lossy().into_owned(),
        });
    }

    /// An in-session launcher event (achievements); dropped without `--ipc`.
    pub fn emit(&self, event: &RunnerEvent) {
        if self.shared.ipc {
            emit_event(event);
        }
    }

    /// Last call before the process exits with `exit_code`; later calls do nothing.
    pub fn finished(&self, exit_code: u8, error: Option<String>) {
        let shared = &*self.shared;
        if !shared.ipc || shared.reported.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(session) = shared.started.get() {
            let play_seconds = session.at.elapsed().as_secs();
            emit_event(&RunnerEvent::Stopped { play_seconds });
            self.write_pending_record(session, play_seconds, exit_code);
        }
        if let Some(message) = error {
            emit_event(&RunnerEvent::Error { message });
        }
    }

    fn write_pending_record(&self, session: &StartedSession, play_seconds: u64, exit_code: u8) {
        let record = PendingSessionRecord {
            rom_path: self.shared.rom.to_string_lossy().into_owned(),
            started_at: i64::try_from(session.unix_ms / 1000).unwrap_or(i64::MAX),
            play_seconds,
            exit_code: i32::from(exit_code),
        };
        let path = session.pending_dir.join(pending_session_file_name(
            session.unix_ms,
            std::process::id(),
        ));
        let written = std::fs::create_dir_all(&session.pending_dir)
            .map_err(anyhow::Error::from)
            .and_then(|()| Ok(serde_json::to_vec(&record)?))
            .and_then(|bytes| Ok(write_atomically(&path, &bytes)?));
        if let Err(err) = written {
            // Play time is lost for this session, the game itself is unaffected.
            tracing::warn!(path = %path.display(), "cannot write pending session: {err:#}");
        }
    }
}
