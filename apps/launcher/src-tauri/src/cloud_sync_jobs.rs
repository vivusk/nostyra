//! Cloud jobs and their bookkeeping, independent of Tauri: one game (pre-play pull
//! with the 15 s budget, post-session push, conflict resolution) or a library pass,
//! each in its own job slot claim, results recorded in `CloudSyncState::status`.

use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use cloud_sync::{RequestBudget, SyncOptions, SyncScope, SyncUnit, SyncUnitError};

use crate::app_events::CLOUD_STATUS_CHANGED;
use crate::app_state::unix_now;
use crate::cloud_game_result::{error_kind, game_result, ErrorKind, SyncGameResult};
use crate::cloud_game_sync::{sync_units, CloudCtx, CloudHost, UnitResult};
use crate::cloud_job_guard::JobGuard;
use crate::cloud_library_pass::{run_pass, PassRequest, PassSummary};
use crate::cloud_provider_factory::CloudAccount;
use crate::cloud_sync_state::{CloudSyncState, JobKind};

/// Pre-play budget: Play never waits longer on the cloud (F3).
pub const PRE_PLAY_BUDGET: Duration = Duration::from_secs(15);
/// How long a background push / resolution waits for a running job.
pub const BACKGROUND_CLAIM_WAIT: Duration = Duration::from_secs(600);

/// One game's units to sync.
#[derive(Debug, Clone)]
pub struct GameSyncRequest {
    pub game_id: i64,
    pub account: CloudAccount,
    pub scope: SyncScope,
    pub units: Vec<SyncUnit>,
    pub options: SyncOptions,
}

/// Runs `request` in a fresh job (blocking). No slot before `deadline` → `Offline`.
pub fn run_game_job<H: CloudHost>(
    host: &H,
    kind: JobKind,
    deadline: Instant,
    stop: Arc<AtomicBool>,
    request: &GameSyncRequest,
) -> SyncGameResult {
    let ctx = host.ctx();
    let Some(guard) = ctx.cloud.claim(kind, deadline, stop) else {
        return SyncGameResult::Offline;
    };
    let units = &request.units;
    let pre_play_account;
    let account = if kind == JobKind::PrePlay {
        pre_play_account = within_deadline(&request.account, deadline);
        &pre_play_account
    } else {
        &request.account
    };
    let results = if kind == JobKind::PrePlay {
        sync_units(&ctx, &guard, account, request.scope, units, request.options)
    } else {
        sync_units_yielding(&ctx, &guard, request)
    };
    drop(guard);
    let result = game_result(account, units, &results);
    record_game_result(ctx.cloud, request.game_id, &result, &results);
    host.notify(
        CLOUD_STATUS_CHANGED,
        serde_json::json!({ "gameId": request.game_id }),
    );
    result
}

/// `account` with every request (token refresh included) held to the pre-play
/// `deadline`: short timeouts, no retry or backoff past it (F3). Local providers
/// are used as is.
pub(crate) fn within_deadline(account: &CloudAccount, deadline: Instant) -> CloudAccount {
    let mut account = account.clone();
    if let Some(provider) = account.provider.budgeted(RequestBudget::until(deadline)) {
        account.provider = provider;
    }
    account
}

/// Background push / resolution: unit by unit, handing the slot to a waiting pre-play
/// pull in between; a unit the pull pre-empted is retried once the pull is done (F9).
fn sync_units_yielding(
    ctx: &CloudCtx<'_>,
    guard: &JobGuard<'_>,
    request: &GameSyncRequest,
) -> Vec<UnitResult> {
    let (account, scope, options) = (&request.account, request.scope, request.options);
    let mut results = Vec::with_capacity(request.units.len());
    for unit in &request.units {
        let result = loop {
            if !guard.yield_to_play() {
                break UnitResult::Stopped;
            }
            let one = sync_units(
                ctx,
                guard,
                account,
                scope,
                std::slice::from_ref(unit),
                options,
            );
            let result = one.into_iter().next().unwrap_or(UnitResult::Stopped);
            let cancelled = matches!(result, UnitResult::Failed(SyncUnitError::Cancelled));
            if !(cancelled && !guard.stopped() && guard.preempted()) {
                break result;
            }
            tracing::info!(target: "cloud_sync", unit = %unit.key(), "pre-empted by a pre-play pull: retried after it");
        };
        results.push(result);
    }
    results
}

/// The pre-play path: the job runs on its own thread; after `budget` its cancel
/// flag is set and `Offline` comes back at once. A pull still `Transferring` only
/// wrote staging files; one already `Applying` is waited for by `play_game`.
pub fn run_within_budget<H: CloudHost>(
    host: &H,
    request: GameSyncRequest,
    budget: Duration,
) -> SyncGameResult {
    let deadline = Instant::now() + budget;
    let stop = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    let (worker_host, worker_stop) = (host.clone(), stop.clone());
    let spawned = std::thread::Builder::new()
        .name("cloud-pre-play".into())
        .spawn(move || {
            let result = run_game_job(
                &worker_host,
                JobKind::PrePlay,
                deadline,
                worker_stop,
                &request,
            );
            let _ = sender.send(result);
        });
    if let Err(err) = spawned {
        tracing::warn!(target: "cloud_sync", %err, "cannot start the pre-play sync");
        return SyncGameResult::Offline;
    }
    match receiver.recv_timeout(budget) {
        Ok(result) => result,
        Err(_) => {
            host.ctx().cloud.stop_job_with(&stop);
            tracing::info!(target: "cloud_sync", "pre-play sync over budget: playing with local saves");
            SyncGameResult::Offline
        }
    }
}

/// Background push / resolution on its own thread (post-session push).
pub fn spawn_game_job<H: CloudHost>(host: H, kind: JobKind, request: GameSyncRequest) {
    let spawned = std::thread::Builder::new()
        .name("cloud-push".into())
        .spawn(move || {
            let deadline = Instant::now() + BACKGROUND_CLAIM_WAIT;
            let stop = Arc::new(AtomicBool::new(false));
            run_game_job(&host, kind, deadline, stop, &request);
        });
    if let Err(err) = spawned {
        tracing::warn!(target: "cloud_sync", %err, "cannot start the cloud push");
    }
}

/// A library pass on its own thread, recorded in the status when it ends. `request`
/// runs on that thread too (library enumeration = fs work, kept off the caller).
pub fn spawn_pass<H, F>(host: H, account: CloudAccount, scope: SyncScope, request: F)
where
    H: CloudHost,
    F: FnOnce() -> PassRequest + Send + 'static,
{
    let spawned = std::thread::Builder::new()
        .name("cloud-pass".into())
        .spawn(move || {
            let summary = run_pass(&host, &account, scope, request());
            record_pass(host.ctx().cloud, &summary);
            host.notify(CLOUD_STATUS_CHANGED, serde_json::json!({}));
        });
    if let Err(err) = spawned {
        tracing::warn!(target: "cloud_sync", %err, "cannot start the cloud pass");
    }
}

/// Stores one game's result (chip, Settings line).
pub fn record_game_result(
    cloud: &CloudSyncState,
    game_id: i64,
    result: &SyncGameResult,
    units: &[UnitResult],
) {
    let mut status = cloud.status();
    if *result == SyncGameResult::Synced {
        status.last_sync_at = Some(unix_now());
    }
    let signed_out = units.iter().any(
        |unit| matches!(unit, UnitResult::Failed(err) if error_kind(err) == ErrorKind::SignIn),
    );
    status.auth_expired = signed_out;
    let text = match result {
        SyncGameResult::Synced => {
            status.conflict_games.remove(&game_id);
            status.game_errors.remove(&game_id);
            "ok".to_string()
        }
        SyncGameResult::Offline => "offline".to_string(),
        SyncGameResult::Conflict { .. } => {
            status.conflict_games.insert(game_id);
            "conflict".to_string()
        }
        SyncGameResult::Skipped { reason } => {
            if reason != "in use" {
                status.game_errors.insert(game_id, reason.clone());
            }
            reason.clone()
        }
    };
    status.last_result = Some(text);
}

/// Stores a pass summary.
pub fn record_pass(cloud: &CloudSyncState, summary: &PassSummary) {
    let mut status = cloud.status();
    if summary.not_started {
        status.last_result = Some("busy".to_string());
        return;
    }
    if !summary.offline {
        status.last_sync_at = Some(unix_now()); // reached the cloud (also: nothing changed)
    }
    status.auth_expired = summary.sign_in_expired;
    status.pending_units = summary.pending_units;
    for game in &summary.clean_games {
        status.conflict_games.remove(game);
        status.game_errors.remove(game);
    }
    status.conflict_games.extend(&summary.conflict_games);
    status.game_errors.extend(summary.errors.clone());
    let text = if summary.sign_in_expired {
        "sign in again".to_string()
    } else if summary.offline {
        "offline".to_string()
    } else if !summary.conflict_games.is_empty() {
        format!("conflicts: {}", summary.conflict_games.len())
    } else if !summary.errors.is_empty() {
        format!("errors: {}", summary.errors.len())
    } else {
        "ok".to_string()
    };
    status.last_result = Some(text);
}
