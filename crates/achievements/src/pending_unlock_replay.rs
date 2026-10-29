//! Replays the offline journal (session start, launcher start): rebuilds each request
//! with `rc_api` (`o=` = seconds since the unlock) and decides from the **server
//! response** what happens to the file: delivered or "already unlocked" → removed;
//! retryable (offline, 5xx, empty) → kept; token rejected → kept, stop, log in again;
//! any other server error → removed + logged (retrying cannot fix it).

use crate::credentials_file::Credentials;
use crate::ffi::{award_request, parse_award_response, parse_submit_response, submit_request};
use crate::offline_response_cache::OfflineResponseCache;
use crate::pending_unlock_journal::{unix_now, PendingEntry, PendingJournal, PendingKind};
use crate::server_transport::{
    is_retryable_response, ServerResponse, ServerTransport, STATUS_NO_RESPONSE,
};
use crate::{RC_EXPIRED_TOKEN, RC_INVALID_CREDENTIALS};

/// Verdict for one replayed entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayOutcome {
    /// Server accepted it (or already had it): remove the file.
    Delivered,
    /// Network / overload: keep the file for the next replay.
    Retry(String),
    /// Token invalid or expired: keep the file; the user must log in again.
    LoginRequired,
    /// Permanent refusal (e.g. achievement not found): remove the file, log why.
    Dropped(String),
}

/// What one replay pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplaySummary {
    pub delivered: usize,
    pub kept: usize,
    pub dropped: usize,
    /// Entries of another RA user (kept untouched).
    pub other_user: usize,
    /// The server rejected the token; the pass stopped early.
    pub login_required: bool,
}

/// Classifies a server answer for an entry of `kind`.
pub(crate) fn classify(kind: PendingKind, response: &ServerResponse) -> ReplayOutcome {
    let parsed = match kind {
        PendingKind::Unlock => parse_award_response(response),
        PendingKind::Leaderboard => parse_submit_response(response),
    };
    match parsed {
        Ok(()) => ReplayOutcome::Delivered,
        Err(_) if is_retryable_response(response) || is_transient_failure(response) => {
            ReplayOutcome::Retry(response.body.clone())
        }
        Err(failure)
            if matches!(failure.code, RC_INVALID_CREDENTIALS | RC_EXPIRED_TOKEN)
                || token_rejected(response) =>
        {
            ReplayOutcome::LoginRequired
        }
        Err(failure) => ReplayOutcome::Dropped(failure.message),
    }
}

/// Failures a retry may fix: HTTP 408 / 5xx, or a body that is not a JSON object
/// (proxy / maintenance page). Only a well-formed JSON error is permanent.
fn is_transient_failure(response: &ServerResponse) -> bool {
    response.status == 408
        || response.status >= 500
        || !serde_json::from_str::<serde_json::Value>(&response.body).is_ok_and(|v| v.is_object())
}

/// The award / submit parsers ignore the JSON `Code` field (only login maps it), so a
/// rejected token is detected here: `Code` `invalid_credentials` / `expired_token`,
/// or HTTP 401.
fn token_rejected(response: &ServerResponse) -> bool {
    if response.status == 401 {
        return true;
    }
    serde_json::from_str::<serde_json::Value>(&response.body)
        .ok()
        .and_then(|json| json.get("Code")?.as_str().map(str::to_owned))
        .is_some_and(|code| code == "invalid_credentials" || code == "expired_token")
}

/// Sends one entry; `None` when the request cannot even be built (bad entry → drop).
fn send(
    entry: &PendingEntry,
    transport: &dyn ServerTransport,
    credentials: &Credentials,
) -> Option<ServerResponse> {
    let seconds_since = u32::try_from((unix_now() - entry.at_unix).max(0)).unwrap_or(u32::MAX);
    let request = match (entry.kind, entry.score) {
        (PendingKind::Unlock, _) => award_request(
            &credentials.user,
            &credentials.token,
            entry.id,
            entry.hardcore,
            &entry.game_hash,
            seconds_since,
        ),
        (PendingKind::Leaderboard, Some(score)) => submit_request(
            &credentials.user,
            &credentials.token,
            entry.id,
            score,
            &entry.game_hash,
            seconds_since,
        ),
        (PendingKind::Leaderboard, None) => return None,
    };
    request.ok().map(|request| transport.call(&request))
}

/// A delivered unlock joins the cached offline session (launcher flush bypasses the
/// offline cache transport), so an offline start does not unlock it again.
fn record_delivered(journal: &PendingJournal, entry: &PendingEntry, credentials: &Credentials) {
    if entry.kind == PendingKind::Unlock {
        OfflineResponseCache::new(journal.data_root()).record_unlock(
            &credentials.user,
            &entry.game_hash,
            entry.id,
            entry.hardcore,
            entry.at_unix,
        );
    }
}

/// Replays every entry of `credentials.user`, oldest first. Stops at the first
/// retryable failure (offline / overloaded: the rest would fail too) or token rejection.
pub fn replay_pending_unlocks(
    journal: &PendingJournal,
    transport: &dyn ServerTransport,
    credentials: &Credentials,
) -> ReplaySummary {
    let mut summary = ReplaySummary::default();
    let entries = journal.list();
    let total = entries.len();
    for (index, (path, entry)) in entries.into_iter().enumerate() {
        if !entry.user.eq_ignore_ascii_case(&credentials.user) {
            summary.other_user += 1;
            continue;
        }
        let (outcome, stop) = match send(&entry, transport, credentials) {
            None => (
                ReplayOutcome::Dropped("invalid journal entry".into()),
                false,
            ),
            Some(response) => {
                let offline = response.status == STATUS_NO_RESPONSE;
                let outcome = classify(entry.kind, &response);
                let stop = offline
                    || matches!(
                        outcome,
                        ReplayOutcome::LoginRequired | ReplayOutcome::Retry(_)
                    );
                (outcome, stop)
            }
        };
        match outcome {
            ReplayOutcome::Delivered | ReplayOutcome::Dropped(_) => {
                if let ReplayOutcome::Dropped(reason) = &outcome {
                    tracing::warn!(id = entry.id, kind = ?entry.kind, %reason, "pending unlock refused by server; dropped");
                    summary.dropped += 1;
                } else {
                    summary.delivered += 1;
                    record_delivered(journal, &entry, credentials);
                }
                if let Err(err) = journal.remove(&path) {
                    tracing::warn!(%err, "cannot remove replayed unlock");
                }
            }
            ReplayOutcome::Retry(_) => summary.kept += 1,
            ReplayOutcome::LoginRequired => {
                summary.kept += 1;
                summary.login_required = true;
            }
        }
        if stop {
            // Everything after this entry stays for the next pass.
            summary.kept += total - index - 1;
            break;
        }
    }
    summary
}

#[cfg(test)]
#[path = "pending_unlock_replay_tests.rs"]
mod tests;
