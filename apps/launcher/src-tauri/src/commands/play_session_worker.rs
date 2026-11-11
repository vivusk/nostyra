//! Follows a running game on a worker thread: forwards its updates to the UI
//! (session, achievements, netplay events), marks a hosted room started (F22),
//! then records play time, frees the session slot and pushes the saves (F14:
//! online guests write nothing, spectators do not play).

use std::path::Path;

use netplay_session::room_api_types::SeatRole;
use runner_session::{
    import_pending_sessions, PendingImportSummary, RunnerSession, SessionOutcome, SessionUpdate,
};
use storage_db::NewPlaySession;
use tauri::{AppHandle, Manager};

use crate::app_events::{
    emit, NetplayEventPayload, SessionEndedPayload, SessionErrorPayload, SessionPayload,
    NETPLAY_EVENT, SESSION_ENDED, SESSION_LAUNCH_TIMEOUT, SESSION_STARTED,
};
use crate::app_state::{lock, AppState};
use crate::cloud_app_host::spawn_push_after_session;
use crate::commands::achievements_commands::on_achievement_update;
use crate::commands::netplay_room_flow::HostStartMarker;

/// The online part of a session the worker needs (`None` = local play).
pub struct SessionNetplay {
    pub role: SeatRole,
    pub start: Option<HostStartMarker>,
}

/// What happens after a session, by netplay seat (`None` = local play).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostSession {
    /// Upload the saves: only a seat that wrote them (local play / host).
    pub push: bool,
    /// Record play time (runner file, else the launcher measurement).
    pub play_time: bool,
}

pub fn post_session(role: Option<SeatRole>) -> PostSession {
    PostSession {
        push: matches!(role, None | Some(SeatRole::Host)),
        play_time: role != Some(SeatRole::Spectator),
    }
}

/// Runs until the runner exits; `spawn_blocking` cannot fail, so the claimed slot is
/// always released by `finish_session`.
pub fn spawn_session_worker(
    app: AppHandle,
    session: RunnerSession,
    game_id: i64,
    mut netplay: Option<SessionNetplay>,
) {
    let pid = session.pid;
    tauri::async_runtime::spawn_blocking(move || {
        for update in session.updates {
            match &update {
                SessionUpdate::Started { .. } => {
                    emit(&app, SESSION_STARTED, SessionPayload { game_id })
                }
                SessionUpdate::LaunchTimedOut => {
                    emit(&app, SESSION_LAUNCH_TIMEOUT, SessionPayload { game_id })
                }
                SessionUpdate::Achievements(update) => on_achievement_update(&app, game_id, update),
                SessionUpdate::Netplay(event) => {
                    let payload = NetplayEventPayload {
                        game_id,
                        event: event.clone(),
                    };
                    emit(&app, NETPLAY_EVENT, payload);
                }
                SessionUpdate::Finished(outcome) => {
                    let role = netplay.as_ref().map(|n| n.role);
                    finish_session(&app, game_id, pid, outcome, role);
                }
            }
            if let Some(start) = netplay.as_mut().and_then(|n| n.start.as_mut()) {
                start.on_update(&update);
            }
        }
    });
}

/// Records play time (runner's pending file first, launcher measurement as a
/// fallback when the runner crashed before writing it) and frees the session slot.
fn finish_session(
    app: &AppHandle,
    game_id: i64,
    pid: u32,
    outcome: &SessionOutcome,
    role: Option<SeatRole>,
) {
    let state = app.state::<AppState>();
    let rules = post_session(role);
    {
        if !rules.play_time {
            discard_pending_of(&state.paths.data_root, pid);
        }
        let conn = state.db();
        let imported =
            import_pending_sessions(&conn, &state.paths.data_root).unwrap_or_else(|err| {
                tracing::warn!(%err, "pending session import failed");
                PendingImportSummary::default()
            });
        let measured = measured_fallback(game_id, outcome, &imported).filter(|_| rules.play_time);
        if let Some(measured) = measured {
            if let Err(err) = storage_db::record_play_session(&conn, &measured) {
                tracing::warn!(%err, "cannot record play session");
            }
        }
    }
    let cancelled = lock(&state.session)
        .take()
        .and_then(|session| session.control)
        .is_some_and(|control| control.was_cancelled());
    let error = outcome
        .error
        .as_ref()
        .filter(|_| !cancelled)
        .map(|err| SessionErrorPayload {
            kind: err.kind,
            message: err.kind.user_message().to_string(),
            detail: err.detail.clone(),
        });
    tracing::info!(game_id, exit_code = ?outcome.exit_code, online = role.is_some(), "game session ended");
    emit(app, SESSION_ENDED, SessionEndedPayload { game_id, error });
    // The slot is free: upload what the session wrote (background, + PS2 memcards).
    if rules.push {
        spawn_push_after_session(app, game_id);
    }
}

/// Removes the play-session file runner `pid` wrote (`<unix ms>-<pid>.json`): a
/// spectator watched, it did not play.
fn discard_pending_of(data_root: &Path, pid: u32) {
    let dir = ipc_protocol::pending_sessions_dir(data_root);
    let suffix = format!("-{pid}.json");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().ends_with(&suffix) {
            if let Err(err) = std::fs::remove_file(entry.path()) {
                tracing::warn!(%err, "cannot drop the spectator's play-session file");
            }
        }
    }
}

/// The launcher-measured session to record when the game started but the runner's
/// own record of *this* session was not imported (runner crashed before writing it).
fn measured_fallback(
    game_id: i64,
    outcome: &SessionOutcome,
    imported: &PendingImportSummary,
) -> Option<NewPlaySession> {
    let started_at = outcome.started_at?;
    if imported.covers_session(game_id, started_at) {
        return None;
    }
    Some(NewPlaySession {
        game_id,
        started_at,
        duration_s: i64::try_from(outcome.measured_seconds).unwrap_or(i64::MAX),
        exit_code: outcome.exit_code,
        source_key: None,
    })
}

#[cfg(test)]
#[path = "play_session_worker_tests.rs"]
mod tests;
