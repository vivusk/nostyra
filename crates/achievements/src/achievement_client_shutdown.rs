//! Shutdown of [`AchievementClient`] in the only safe order (spike D4): firing an
//! award / submit / ping callback after `rc_client_destroy` is a use-after-free, and an
//! award's first −2 re-sends immediately, so pending requests are failed in a loop.

use crate::achievement_client::AchievementClient;
use crate::achievement_events::ClientCompletion;
use crate::request_queue::WORKER_JOIN_TIMEOUT;
use crate::server_transport::BODY_SHUTDOWN;

/// Rounds of "fail pending → unload" at shutdown before giving up (2 always suffice).
const MAX_SHUTDOWN_ROUNDS: usize = 8;

/// What shutdown did, for logs and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownReport {
    /// Requests issued over the client's life.
    pub requests_issued: u64,
    /// Callbacks fired over the client's life (equals `requests_issued` after shutdown).
    pub callbacks_fired: u64,
    /// Callbacks fired with −2 `shutdown` during shutdown.
    pub failed_at_shutdown: usize,
    /// Completions (e.g. a login failed by shutdown) not drained before shutdown.
    pub undrained_completions: Vec<ClientCompletion>,
    /// True when the HTTP worker was still blocked after 2 s and was detached.
    pub worker_detached: bool,
}

impl AchievementClient {
    /// Ends the client in the only safe order (D4): deliver arrived results, then
    /// repeat {fire −2 for every pending request → unload game} until nothing is
    /// pending, then `rc_client_destroy`, then join the worker (≤ 2 s, else detach).
    pub fn shutdown(mut self) -> ShutdownReport {
        self.shutdown_in_place()
    }

    pub(crate) fn shutdown_in_place(&mut self) -> ShutdownReport {
        let Some(raw) = self.raw.take() else {
            unreachable!("shutdown runs once: `shutdown` consumes self, Drop skips when taken")
        };
        let queue = &raw.state().queue;
        queue.complete_ready();
        let mut failed_at_shutdown = 0;
        for _ in 0..MAX_SHUTDOWN_ROUNDS {
            failed_at_shutdown += queue.fail_all_pending(BODY_SHUTDOWN);
            raw.unload_game();
            if queue.pending_count() == 0 {
                break;
            }
        }
        let undrained_completions = std::mem::take(&mut *raw.state().completions.borrow_mut());
        let mut state = raw.destroy();
        let (requests_issued, callbacks_fired) = state.queue.counters();
        let worker_detached = state.queue.close_worker(WORKER_JOIN_TIMEOUT);
        ShutdownReport {
            requests_issued,
            callbacks_fired,
            failed_at_shutdown,
            undrained_completions,
            worker_detached,
        }
    }
}

impl Drop for AchievementClient {
    fn drop(&mut self) {
        if self.raw.is_some() {
            self.shutdown_in_place();
        }
    }
}
