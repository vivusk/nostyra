//! The cloud-sync slot of `AppState`: at most one sync job at a time (transfers are
//! sequential per account), the units it is working on right now, the last results
//! for Settings / the Detail chip, the cached account and the closing flag.
//!
//! Lock order: **session → cloud job** (never the other way). A job only locks the
//! session while entering a unit (`JobGuard::enter_unit`), Play and the folder
//! writers hold the session and look at the job briefly (`cloud_unit_guard`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use cloud_sync::SyncProgress;
use serde::Serialize;

use crate::app_state::lock;
use crate::cloud_job_guard::{new_job, JobGuard};
use crate::cloud_provider_factory::CachedAccount;

/// What started a job (pre-play pulls jump the queue, see [`JobGuard::yield_to_play`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    PrePlay,
    Push,
    Pass,
}

/// The running job.
pub(crate) struct CloudJob {
    pub(crate) id: u64,
    pub(crate) kind: JobKind,
    /// Units being synced right now (guards check these).
    pub(crate) unit_keys: BTreeSet<String>,
    /// Phase + cancel of the current unit sync.
    pub(crate) progress: Arc<SyncProgress>,
    /// Stops the whole job (budget expiry, close-wait): remaining units go next time.
    pub(crate) stop: Arc<AtomicBool>,
    /// A pre-play pull cancelled the current unit: the job retries it after the pull.
    pub(crate) preempted: bool,
}

/// Last results, shown in Settings and on the Detail chip.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudStatus {
    /// Unix seconds of the last finished sync.
    pub last_sync_at: Option<i64>,
    /// "ok", "offline", "conflicts: 2", an error message…
    pub last_result: Option<String>,
    /// Units left unsynced by the last pass / push (offline, in use, stopped).
    pub pending_units: usize,
    /// Games whose conflicts wait for the user's choice.
    pub conflict_games: BTreeSet<i64>,
    /// Per-game error of the last sync (chip tooltip).
    pub game_errors: BTreeMap<i64, String>,
    /// The provider refused the stored sign-in (`AuthExpired`).
    pub auth_expired: bool,
}

/// Close-wait progress (`cloud_close_wait`).
pub const CLOSE_IDLE: u8 = 0;
pub const CLOSE_WAITING: u8 = 1;
pub const CLOSE_DONE: u8 = 2;

#[derive(Default)]
pub struct CloudSyncState {
    job: Mutex<Option<CloudJob>>,
    pub(crate) job_changed: Condvar,
    next_id: AtomicU64,
    pub(crate) play_waiters: AtomicUsize,
    pub account: Mutex<Option<CachedAccount>>,
    pub status: Mutex<CloudStatus>,
    /// [`CLOSE_IDLE`] / [`CLOSE_WAITING`] / [`CLOSE_DONE`]; no new job once waiting.
    pub closing: AtomicU8,
    /// Cancel flag of a running OAuth sign-in (HTTP providers, phases 04–06).
    pub sign_in_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl CloudSyncState {
    /// Takes the job slot, waiting until `deadline` for a running job. `stop` = the
    /// caller's flag for the whole job. `None`: deadline passed or the app is closing.
    /// A pre-play pull pre-empts a background job instead of waiting it out (any
    /// game's job: Play is never blocked by an upload): its current unit is cancelled,
    /// it hands the slot over and takes it back after the pull (a push retries the
    /// cancelled unit, a pass leaves it to the next one). Only a running `Applying` is
    /// waited for; no pre-empt once `deadline` passed (the pull gets no slot anyway).
    pub fn claim(
        &self,
        kind: JobKind,
        deadline: Instant,
        stop: Arc<AtomicBool>,
    ) -> Option<JobGuard<'_>> {
        let play = kind == JobKind::PrePlay;
        if play {
            self.play_waiters.fetch_add(1, Ordering::SeqCst);
        }
        let mut job = self.lock_job();
        let mut claimed = None;
        loop {
            if self.closing.load(Ordering::SeqCst) != CLOSE_IDLE || stop.load(Ordering::SeqCst) {
                break;
            }
            // A background job waits while a pre-play pull is queued.
            let waiting_play = !play && self.play_waiters.load(Ordering::SeqCst) > 0;
            let now = Instant::now();
            if job.is_none() && !waiting_play {
                let id = self.next_id.fetch_add(1, Ordering::SeqCst);
                *job = Some(new_job(id, kind, stop.clone()));
                claimed = Some(id);
                break;
            }
            if now >= deadline {
                break;
            }
            if play {
                preempt_background(job.as_mut());
            }
            let wait = (deadline - now).min(Duration::from_millis(200));
            job = self
                .job_changed
                .wait_timeout(job, wait)
                .map_or_else(|p| p.into_inner().0, |r| r.0);
        }
        drop(job);
        if play {
            self.play_waiters.fetch_sub(1, Ordering::SeqCst);
            self.job_changed.notify_all();
        }
        claimed.map(|id| JobGuard::new(self, id, kind, stop))
    }

    /// Whether any job runs.
    pub fn job_running(&self) -> bool {
        self.lock_job().is_some()
    }

    /// Progress of the running job when it works on any of `keys`.
    pub fn job_progress_for(&self, keys: &BTreeSet<String>) -> Option<Arc<SyncProgress>> {
        let job = self.lock_job();
        job.as_ref()
            .filter(|job| !job.unit_keys.is_disjoint(keys))
            .map(|job| job.progress.clone())
    }

    /// Stops the running job: its stop flag + cancel of the current unit. Returns the
    /// unit's progress (the caller waits for a running `Applying` to end).
    pub fn stop_running(&self) -> Option<Arc<SyncProgress>> {
        let job = self.lock_job();
        let job = job.as_ref()?;
        job.stop.store(true, Ordering::SeqCst);
        job.progress.cancel();
        Some(job.progress.clone())
    }

    /// Stops the job only when it is the one owning `stop` (a pre-play budget expiry).
    pub fn stop_job_with(&self, stop: &Arc<AtomicBool>) {
        stop.store(true, Ordering::SeqCst);
        let job = self.lock_job();
        if let Some(job) = job.as_ref().filter(|job| Arc::ptr_eq(&job.stop, stop)) {
            job.progress.cancel();
        }
    }

    /// Waits until no job runs; `false` after `timeout`.
    pub fn wait_for_no_job(&self, timeout: Duration) -> bool {
        let job = self.lock_job();
        let (job, _) = self
            .job_changed
            .wait_timeout_while(job, timeout, |job| job.is_some())
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        job.is_none()
    }

    pub fn status(&self) -> MutexGuard<'_, CloudStatus> {
        lock(&self.status)
    }

    pub(crate) fn lock_job(&self) -> MutexGuard<'_, Option<CloudJob>> {
        lock(&self.job)
    }
}

/// Cancels the current unit of a running background job for a waiting pre-play pull
/// (repeated each wait round, so a unit entered meanwhile is cancelled too). The job
/// is not stopped: it yields between units ([`JobGuard::yield_to_play`]). The cancel
/// cannot interrupt an `Applying` unit; the caller simply keeps waiting for the slot.
fn preempt_background(job: Option<&mut CloudJob>) {
    let Some(job) = job.filter(|job| job.kind != JobKind::PrePlay) else {
        return;
    };
    job.preempted = true;
    job.progress.cancel();
}

#[cfg(test)]
#[path = "cloud_sync_state_tests.rs"]
mod tests;
