//! Closing the launcher while a cloud job runs (F7): the window close is prevented,
//! the UI gets `cloud-closing-wait` (modal "Uploading saves…"), and a blocking task
//! (never the event loop) waits up to 15 s for the job. On timeout the job is
//! cancelled (a cancelled push writes no manifest: re-pushed at next start) and a
//! running `Applying` pass is waited for without cutoff (exiting mid-rename could
//! leave a half-applied folder); then the app exits. A second close click while
//! waiting is ignored.

use std::sync::atomic::Ordering;
use std::time::Duration;

use cloud_sync::SyncPhase;
use tauri::{AppHandle, Manager};

use crate::app_events::{emit, CLOUD_CLOSING_WAIT};
use crate::app_state::AppState;
use crate::cloud_sync_state::{CloudSyncState, CLOSE_DONE, CLOSE_IDLE, CLOSE_WAITING};

/// How long closing waits for a running job before cancelling it.
pub const CLOSE_WAIT: Duration = Duration::from_secs(15);
const APPLY_POLL: Duration = Duration::from_millis(5);

/// What a close request does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    /// No job: close normally.
    CloseNow,
    /// A job runs: prevent the close and start the wait task.
    StartWait,
    /// The wait task already runs: prevent the close, nothing else.
    AlreadyWaiting,
}

/// Decides and records the close state. Taking `CLOSE_WAITING` first means no new
/// job can be claimed between the "is a job running" check and the exit.
pub fn close_decision(cloud: &CloudSyncState) -> CloseDecision {
    let claimed = cloud.closing.compare_exchange(
        CLOSE_IDLE,
        CLOSE_WAITING,
        Ordering::SeqCst,
        Ordering::SeqCst,
    );
    match claimed {
        Err(CLOSE_DONE) => CloseDecision::CloseNow,
        Err(_) => CloseDecision::AlreadyWaiting,
        Ok(_) if cloud.job_running() => CloseDecision::StartWait,
        Ok(_) => {
            cloud.closing.store(CLOSE_DONE, Ordering::SeqCst);
            CloseDecision::CloseNow
        }
    }
}

/// How the wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitOutcome {
    /// The job finished within the timeout.
    Idle,
    /// Timed out: the job was cancelled (and its apply pass, if any, waited for).
    Cancelled,
}

/// Waits up to `timeout` for the running job, then cancels it and waits for a
/// running `Applying` pass to end. Marks the close as done.
pub fn wait_for_idle(cloud: &CloudSyncState, timeout: Duration) -> WaitOutcome {
    let outcome = if cloud.wait_for_no_job(timeout) {
        WaitOutcome::Idle
    } else {
        if let Some(progress) = cloud.stop_running() {
            while progress.phase() == SyncPhase::Applying {
                std::thread::sleep(APPLY_POLL);
            }
        }
        WaitOutcome::Cancelled
    };
    cloud.closing.store(CLOSE_DONE, Ordering::SeqCst);
    outcome
}

/// Main window `CloseRequested` handler.
pub fn on_close_requested(app: &AppHandle, api: &tauri::CloseRequestApi) {
    let state = app.state::<AppState>();
    match close_decision(&state.cloud) {
        CloseDecision::CloseNow => {}
        CloseDecision::AlreadyWaiting => api.prevent_close(),
        CloseDecision::StartWait => {
            api.prevent_close();
            emit(app, CLOUD_CLOSING_WAIT, ());
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let state = app.state::<AppState>();
                let outcome = wait_for_idle(&state.cloud, CLOSE_WAIT);
                tracing::info!(target: "cloud_sync", ?outcome, "closing after cloud sync");
                app.exit(0);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Instant;

    use super::*;
    use crate::cloud_sync_state::JobKind;

    fn stop() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    #[test]
    fn no_job_closes_at_once() {
        let cloud = CloudSyncState::default();
        assert_eq!(close_decision(&cloud), CloseDecision::CloseNow);
        assert_eq!(close_decision(&cloud), CloseDecision::CloseNow);
    }

    #[test]
    fn job_finishing_early_ends_the_wait() {
        let cloud = Arc::new(CloudSyncState::default());
        let (claimed, release) = std::sync::mpsc::channel();
        let worker = {
            let cloud = cloud.clone();
            std::thread::spawn(move || {
                let guard = cloud.claim(JobKind::Push, Instant::now(), stop()).unwrap();
                claimed.send(()).unwrap();
                std::thread::sleep(Duration::from_millis(100));
                drop(guard);
            })
        };
        release.recv().unwrap();
        assert_eq!(close_decision(&cloud), CloseDecision::StartWait);
        assert_eq!(
            close_decision(&cloud),
            CloseDecision::AlreadyWaiting,
            "second click"
        );
        let started = Instant::now();
        assert_eq!(
            wait_for_idle(&cloud, Duration::from_secs(5)),
            WaitOutcome::Idle
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        worker.join().unwrap();
        assert_eq!(
            close_decision(&cloud),
            CloseDecision::CloseNow,
            "after the wait"
        );
        assert!(
            cloud.claim(JobKind::Push, Instant::now(), stop()).is_none(),
            "no job after close"
        );
    }

    #[test]
    fn job_never_finishing_is_cancelled_after_the_timeout() {
        let cloud = CloudSyncState::default();
        let job_stop = stop();
        let guard = cloud
            .claim(JobKind::Pass, Instant::now(), job_stop.clone())
            .unwrap();
        let session = std::sync::Mutex::new(None);
        let progress = guard.enter_unit(&session, "snes/abc").unwrap();
        assert_eq!(close_decision(&cloud), CloseDecision::StartWait);
        let started = Instant::now();
        let outcome = wait_for_idle(&cloud, Duration::from_millis(100));
        assert_eq!(outcome, WaitOutcome::Cancelled);
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(progress.is_cancelled() && job_stop.load(Ordering::SeqCst));
        drop(guard);
    }
}
