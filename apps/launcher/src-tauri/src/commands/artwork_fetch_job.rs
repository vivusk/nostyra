//! The artwork fetch job: one at a time, on a worker thread with a 4-thread pool.
//! Requests arriving while it runs are queued (rerun flag / game list) and served by
//! another pass at the end, decided under the slot lock so none is lost.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use artwork_cache::{fetch_game_artwork, GameArtRequest, ThumbnailIndex};
use game_library::display_title;
use rayon::prelude::*;
use tauri::{AppHandle, Manager};

use crate::app_events::{
    emit, ArtworkFinishedPayload, ArtworkProgressPayload, SessionPayload, ARTWORK_FINISHED,
    ARTWORK_PROGRESS, ARTWORK_UPDATED,
};
use crate::app_state::{lock, unix_now, AppState};
use crate::command_error::{CommandError, CommandResult};
use crate::commands::artwork_job_accounting::{artwork_record, count_outcome, TransientBreaker};
use crate::commands::artwork_work_list::{game_artwork_requests, missing_artwork_requests};

/// Parallel downloads (thumbnails are small; the breaker stops dead-network churn).
const ARTWORK_WORKERS: usize = 4;

/// Starts a job for all missing art (`None`) or one game; returns at once. While a job
/// runs the request is queued instead.
pub fn start_artwork_job(app: &AppHandle, game_id: Option<i64>) -> CommandResult<()> {
    let state = app.state::<AppState>();
    let cancel = {
        let mut slot = lock(&state.artwork_cancel);
        if slot.is_some() {
            match game_id {
                None => state.artwork_rerun.store(true, Ordering::Relaxed),
                Some(id) => lock(&state.artwork_queued_games).push(id),
            }
            return Ok(());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(cancel.clone());
        cancel
    };
    let started = ArtworkProgressPayload::default();
    *lock(&state.artwork_progress) = Some(started.clone());
    emit(app, ARTWORK_PROGRESS, started);
    let worker_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || run_job(&worker_app, &cancel, game_id));
    Ok(())
}

fn run_job(app: &AppHandle, cancel: &AtomicBool, first_game: Option<i64>) {
    let state = app.state::<AppState>();
    let breaker = TransientBreaker::default();
    let mut totals = ArtworkFinishedPayload::default();
    let mut errors = Vec::new();
    let (mut missing, mut games) = (first_game.is_none(), Vec::from_iter(first_game));
    loop {
        if let Err(err) = run_pass(app, &state, cancel, &breaker, missing, &games, &mut totals) {
            tracing::warn!(err = %err.message, "artwork pass failed");
            errors.push(err.message);
        }
        // Under the slot lock: a request queued by start_artwork_job is either seen
        // here or starts a fresh job — never lost.
        let mut slot = lock(&state.artwork_cancel);
        if !cancel.load(Ordering::Relaxed) {
            missing = state.artwork_rerun.swap(false, Ordering::Relaxed);
            games = std::mem::take(&mut *lock(&state.artwork_queued_games));
            if missing || !games.is_empty() {
                continue;
            }
        }
        state.artwork_rerun.store(false, Ordering::Relaxed);
        lock(&state.artwork_queued_games).clear();
        *lock(&state.artwork_progress) = None;
        *slot = None;
        break;
    }
    if breaker.tripped() {
        tracing::info!(
            transient = totals.transient,
            "artwork fetch stopped: network unavailable"
        );
    }
    totals.cancelled = cancel.load(Ordering::Relaxed) && !breaker.tripped();
    totals.error = (!errors.is_empty()).then(|| errors.join("\n"));
    emit(app, ARTWORK_FINISHED, totals);
}

/// One pass over a fresh work list.
fn run_pass(
    app: &AppHandle,
    state: &AppState,
    cancel: &AtomicBool,
    breaker: &TransientBreaker,
    missing: bool,
    games: &[i64],
    totals: &mut ArtworkFinishedPayload,
) -> CommandResult<()> {
    let mut work = if missing {
        missing_artwork_requests(&state.db(), unix_now())?
    } else {
        Vec::new()
    };
    let extra = game_artwork_requests(&state.db(), games)?;
    for request in extra {
        if work.iter().all(|w| w.game_id != request.game_id) {
            work.push(request);
        }
    }
    if work.is_empty() {
        return Ok(());
    }
    let mut indexes = HashMap::new();
    for request in &work {
        let system = request.system;
        indexes
            .entry(system.id)
            .or_insert_with(|| thumbnail_index(state, system));
    }
    let done = AtomicUsize::new(0);
    let pass_totals = std::sync::Mutex::new(ArtworkFinishedPayload::default());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(ARTWORK_WORKERS)
        .build()
        .map_err(|err| CommandError::new("artwork", err.to_string()))?;
    pool.install(|| {
        work.par_iter().for_each(|request| {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let dir = &state.paths.artwork_dir;
            let index = indexes.get(request.system.id).and_then(Option::as_deref);
            let outcome = fetch_game_artwork(&state.http, index, request, dir, cancel);
            if breaker.record(&outcome) {
                cancel.store(true, Ordering::Relaxed);
            }
            count_outcome(&mut lock(&pass_totals), &outcome);
            if store_outcome(state, request, &outcome) {
                emit(
                    app,
                    ARTWORK_UPDATED,
                    SessionPayload {
                        game_id: request.game_id,
                    },
                );
            }
            let payload = ArtworkProgressPayload {
                done: done.fetch_add(1, Ordering::Relaxed) + 1,
                total: work.len(),
                current_title: Some(display_title(&request.no_intro_name)),
            };
            *lock(&state.artwork_progress) = Some(payload.clone());
            emit(app, ARTWORK_PROGRESS, payload);
        });
    });
    let pass = lock(&pass_totals).clone();
    totals.cached += pass.cached;
    totals.not_found += pass.not_found;
    totals.failed += pass.failed;
    totals.transient += pass.transient;
    Ok(())
}

/// Writes the decided kinds of one game; returns whether any row changed.
fn store_outcome(
    state: &AppState,
    request: &GameArtRequest,
    outcome: &artwork_cache::ArtworkOutcome,
) -> bool {
    let mut wrote = false;
    let checked_at = unix_now();
    for (kind, kind_outcome) in &outcome.results {
        let Some(record) = artwork_record(request.game_id, *kind, kind_outcome, checked_at) else {
            continue;
        };
        match storage_db::upsert_artwork(&state.db(), &record) {
            Ok(()) => wrote = true,
            Err(err) => tracing::warn!(game_id = request.game_id, %err, "artwork row not saved"),
        }
    }
    wrote
}

/// `system`'s thumbnail index, loaded (cache or one network refresh) on first use.
/// `None` (offline, no cache) is not remembered: fetches fall back to direct GETs.
fn thumbnail_index(
    state: &AppState,
    system: &'static system_catalog::SystemSpec,
) -> Option<Arc<ThumbnailIndex>> {
    let mut indexes = lock(&state.thumbnail_indexes);
    if let Some(index) = indexes.get(system.id) {
        return Some(index.clone());
    }
    let dir = &state.paths.artwork_dir;
    let loaded = ThumbnailIndex::load_or_fetch(&state.http, dir, system, SystemTime::now())?;
    let loaded = Arc::new(loaded);
    indexes.insert(system.id, loaded.clone());
    Some(loaded)
}
