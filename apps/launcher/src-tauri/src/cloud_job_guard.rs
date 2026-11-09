//! The claimed cloud job slot ([`JobGuard`]): enters / leaves units under the
//! session → job lock order, yields to a waiting pre-play pull between units of a
//! background job, frees the slot on drop.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cloud_sync::SyncProgress;

use crate::app_state::{lock, ActiveSession};
use crate::cloud_sync_state::{CloudJob, CloudSyncState, JobKind, CLOSE_IDLE};

/// Why a unit was not entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitRefused {
    /// A running game holds the unit.
    InUse,
    /// The job was stopped.
    Stopped,
}

pub(crate) fn new_job(id: u64, kind: JobKind, stop: Arc<AtomicBool>) -> CloudJob {
    CloudJob {
        id,
        kind,
        unit_keys: BTreeSet::new(),
        progress: Arc::new(SyncProgress::new()),
        stop,
        preempted: false,
    }
}

/// The claimed job slot; dropping it frees the slot.
pub struct JobGuard<'a> {
    cloud: &'a CloudSyncState,
    id: u64,
    kind: JobKind,
    stop: Arc<AtomicBool>,
}

impl<'a> JobGuard<'a> {
    pub(crate) fn new(
        cloud: &'a CloudSyncState,
        id: u64,
        kind: JobKind,
        stop: Arc<AtomicBool>,
    ) -> Self {
        Self {
            cloud,
            id,
            kind,
            stop,
        }
    }

    /// Marks `key` as the unit being synced, unless a running game holds it (lock
    /// order session → job) or the job was stopped. Returns a fresh progress handle.
    pub fn enter_unit(
        &self,
        session: &Mutex<Option<ActiveSession>>,
        key: &str,
    ) -> Result<Arc<SyncProgress>, UnitRefused> {
        let session = lock(session);
        let mut job = self.cloud.lock_job();
        let job = job.as_mut().filter(|job| job.id == self.id);
        let Some(job) = job.filter(|_| !self.stopped()) else {
            return Err(UnitRefused::Stopped);
        };
        if session.as_ref().is_some_and(|s| s.unit_keys.contains(key)) {
            return Err(UnitRefused::InUse);
        }
        job.unit_keys = BTreeSet::from([key.to_string()]);
        job.progress = Arc::new(SyncProgress::new());
        job.preempted = false;
        Ok(job.progress.clone())
    }

    /// Whether a pre-play pull cancelled the current unit (not Play of the unit's own
    /// game, not a stop): the caller retries the unit after [`Self::yield_to_play`].
    pub fn preempted(&self) -> bool {
        let job = self.cloud.lock_job();
        job.as_ref()
            .is_some_and(|job| job.id == self.id && job.preempted)
    }

    /// The current unit is done.
    pub fn leave_unit(&self) {
        if let Some(job) = self
            .cloud
            .lock_job()
            .as_mut()
            .filter(|job| job.id == self.id)
        {
            job.unit_keys.clear();
        }
    }

    /// Whether the whole job was asked to stop.
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Between units of a background job: when a pre-play pull waits, hands the slot
    /// over and takes it back afterwards. `false` = stopped / closing meanwhile.
    pub fn yield_to_play(&self) -> bool {
        let cloud = self.cloud;
        if cloud.play_waiters.load(Ordering::SeqCst) == 0 {
            return !self.stopped();
        }
        let mut job = cloud.lock_job();
        *job = None;
        cloud.job_changed.notify_all();
        loop {
            if cloud.closing.load(Ordering::SeqCst) != CLOSE_IDLE || self.stopped() {
                return false;
            }
            if job.is_none() && cloud.play_waiters.load(Ordering::SeqCst) == 0 {
                *job = Some(new_job(self.id, self.kind, self.stop.clone()));
                return true;
            }
            job = cloud
                .job_changed
                .wait_timeout(job, Duration::from_millis(200))
                .map_or_else(|p| p.into_inner().0, |r| r.0);
        }
    }
}

impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        let mut job = self.cloud.lock_job();
        if job.as_ref().is_some_and(|job| job.id == self.id) {
            *job = None;
        }
        drop(job);
        self.cloud.job_changed.notify_all();
    }
}
