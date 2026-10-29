//! Launcher-start replay of the offline unlock journal (`<data>/achievements/pending`):
//! unlocks earned while offline reach RetroAchievements even if the game is not played
//! again. Runs on its own thread after app state init (never blocks the UI); skipped
//! with `NOSTYRA_OFFLINE=1`, when logged out, when nothing is pending and while a game
//! runs (that runner replays the journal itself). Other users' entries stay untouched.

use std::path::Path;

use achievements::{
    load_credentials, replay_pending_unlocks, PendingJournal, ReplaySummary, ServerTransport,
    UreqTransport,
};
use tauri::{AppHandle, Manager};

use crate::app_state::{lock, AppState};
use crate::commands::achievements_commands::{handle_login_expired, launcher_user_agent};

/// What one flush did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlushResult {
    Skipped(&'static str),
    Replayed(ReplaySummary),
}

/// Starts the background flush (no-op offline).
pub fn spawn_startup_flush(app: &AppHandle) {
    if std::env::var("NOSTYRA_OFFLINE").is_ok_and(|value| value == "1") {
        tracing::info!("achievement journal flush skipped: offline mode");
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("ra-journal-flush".into())
        .spawn(move || {
            let state = app.state::<AppState>();
            let session_active = lock(&state.session).is_some();
            let transport = UreqTransport::new(&launcher_user_agent());
            let result = flush_journal(&state.paths.data_root, &transport, session_active);
            tracing::info!(?result, "achievement journal flush");
            if matches!(&result, FlushResult::Replayed(summary) if summary.login_required) {
                handle_login_expired(&app);
            }
        });
    if let Err(err) = spawned {
        tracing::warn!(%err, "cannot start the achievement journal flush");
    }
}

/// Replays the journal for the stored user unless a skip rule applies.
pub fn flush_journal(
    data_root: &Path,
    transport: &dyn ServerTransport,
    session_active: bool,
) -> FlushResult {
    if session_active {
        return FlushResult::Skipped("a game is running");
    }
    let credentials = match load_credentials(data_root) {
        Ok(Some(credentials)) => credentials,
        Ok(None) => return FlushResult::Skipped("logged out"),
        Err(err) => {
            tracing::warn!(%err, "unreadable achievements credentials file");
            return FlushResult::Skipped("unreadable credentials");
        }
    };
    let journal = PendingJournal::new(data_root);
    if journal.list().is_empty() {
        return FlushResult::Skipped("nothing pending");
    }
    FlushResult::Replayed(replay_pending_unlocks(&journal, transport, &credentials))
}

#[cfg(test)]
#[path = "achievements_journal_flush_tests.rs"]
mod tests;
