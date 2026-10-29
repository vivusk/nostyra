//! Client-thread registry of in-flight rcheevos requests (`RequestId → callback`) and
//! the dispatch to a transport: inline (answered inside `server_call`, used by
//! in-process test suites) or a FIFO HTTP worker thread that only ever sees `Send`
//! data `(id, request)` and returns `(id, response)`. Every callback fires exactly once,
//! on the client thread; results for unknown ids (late, after shutdown) are dropped.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use crate::ffi::PendingCallback;
use crate::request_http_worker::{spawn_worker, HttpWorker, TransportJob, WorkItem};
use crate::request_observer::{track, RequestObservation, RequestObserver, TrackedSubmission};
use crate::server_transport::{
    ServerRequest, ServerResponse, ServerTransport, BODY_OFFLINE, STATUS_NO_RESPONSE,
};

/// Identifies one in-flight request inside a [`RequestQueue`].
pub(crate) type RequestId = u64;

/// Where requests are executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestDispatch {
    /// Transport called inside `server_call` on the client thread; the callback fires
    /// before `server_call` returns (deterministic frame timing in tests).
    Inline,
    /// One background thread runs the transport; results are delivered by
    /// [`crate::AchievementClient::complete_ready_requests`].
    Worker,
}

/// Rounds of "fire −2 for everything pending" before giving up: an award's first −2
/// re-sends once immediately, so two rounds always suffice; the cap guards a bug.
const MAX_FAIL_ROUNDS: usize = 16;
/// How long shutdown waits for a busy worker before detaching it.
pub(crate) const WORKER_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

enum Mode {
    Inline(Arc<dyn ServerTransport>),
    Worker(HttpWorker),
}

/// Registry + dispatch. `!Send` (holds [`PendingCallback`]s): lives on the client thread.
pub(crate) struct RequestQueue {
    registry: RefCell<BTreeMap<RequestId, (PendingCallback, Option<TrackedSubmission>)>>,
    observer: RequestObserver,
    next_id: Cell<RequestId>,
    issued: Cell<u64>,
    fired: Cell<u64>,
    mode: Mode,
}

impl RequestQueue {
    pub(crate) fn new(transport: Arc<dyn ServerTransport>, dispatch: RequestDispatch) -> Self {
        let mode = match dispatch {
            RequestDispatch::Inline => Mode::Inline(transport),
            RequestDispatch::Worker => Mode::Worker(spawn_worker(transport)),
        };
        Self {
            registry: RefCell::new(BTreeMap::new()),
            observer: RequestObserver::default(),
            next_id: Cell::new(0),
            issued: Cell::new(0),
            fired: Cell::new(0),
            mode,
        }
    }

    /// Registers `callback` and dispatches `request`. Inline mode completes it now.
    pub(crate) fn submit(&self, request: ServerRequest, callback: PendingCallback) {
        let id = self.next_id.get() + 1;
        self.next_id.set(id);
        self.issued.set(self.issued.get() + 1);
        let tracked = track(&request);
        if let Some(tracked) = &tracked {
            self.observer.sent(tracked);
        }
        self.registry.borrow_mut().insert(id, (callback, tracked));
        match &self.mode {
            Mode::Inline(transport) => {
                let response = transport.call(&request);
                self.complete(id, &response);
            }
            Mode::Worker(worker) => {
                let sent = worker
                    .to_worker
                    .as_ref()
                    .is_some_and(|tx| tx.send(WorkItem::Request { id, request }).is_ok());
                if !sent {
                    self.complete(id, &ServerResponse::no_response(BODY_OFFLINE));
                }
            }
        }
    }

    /// Fires the callback registered under `id`; false for unknown / already fired ids.
    pub(crate) fn complete(&self, id: RequestId, response: &ServerResponse) -> bool {
        // Borrow released before firing: the callback may re-enter `submit`.
        let Some((callback, tracked)) = self.registry.borrow_mut().remove(&id) else {
            return false;
        };
        if let Some(tracked) = tracked {
            self.observer.answered(tracked, response); // before a retry's `Sent`
        }
        self.fired.set(self.fired.get() + 1);
        callback.fire(response);
        true
    }

    /// Delivers every worker result that has arrived (non-blocking). Returns how many
    /// callbacks fired.
    pub(crate) fn complete_ready(&self) -> usize {
        let Mode::Worker(worker) = &self.mode else {
            return 0;
        };
        let mut fired = 0;
        while let Ok(result) = worker.from_worker.try_recv() {
            fired += usize::from(self.complete(result.id, &result.response));
        }
        fired
    }

    /// Completes every pending request with status −2 and `reason`, including
    /// requests the callbacks re-submit meanwhile. Returns how many fired.
    pub(crate) fn fail_all_pending(&self, reason: &str) -> usize {
        let response = ServerResponse {
            status: STATUS_NO_RESPONSE,
            body: reason.to_owned(),
        };
        let mut fired = 0;
        for _ in 0..MAX_FAIL_ROUNDS {
            let ids: Vec<RequestId> = self.registry.borrow().keys().copied().collect();
            if ids.is_empty() {
                break;
            }
            for id in ids {
                fired += usize::from(self.complete(id, &response));
            }
        }
        fired
    }

    /// Runs `job` on the worker FIFO, `background` = after everything else queued
    /// (inline mode: now, on this thread).
    pub(crate) fn run_job(&self, job: TransportJob, background: bool) {
        let item = |job| match background {
            true => WorkItem::BackgroundJob(job),
            false => WorkItem::Job(job),
        };
        match &self.mode {
            Mode::Inline(transport) => job(transport.as_ref()),
            Mode::Worker(worker) => match &worker.to_worker {
                Some(tx) => drop(tx.send(item(job))),
                None => tracing::warn!("achievement HTTP worker unavailable; job skipped"),
            },
        }
    }

    /// Award / submit traffic since the last drain, in order.
    pub(crate) fn drain_observations(&self) -> Vec<RequestObservation> {
        self.observer.drain()
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.registry.borrow().len()
    }

    /// (requests issued, callbacks fired) since creation.
    pub(crate) fn counters(&self) -> (u64, u64) {
        (self.issued.get(), self.fired.get())
    }

    /// Closes the worker channel and joins the worker for up to `timeout`; a worker
    /// stuck in a blocking call is detached (it holds no callback). Returns true when
    /// detached. Idempotent; a no-op in inline mode.
    pub(crate) fn close_worker(&mut self, timeout: Duration) -> bool {
        match &mut self.mode {
            Mode::Worker(worker) => worker.close(timeout),
            Mode::Inline(_) => false,
        }
    }
}

impl Drop for RequestQueue {
    fn drop(&mut self) {
        let leftover = self.pending_count();
        if leftover > 0 {
            // Never fired here: the client may already be destroyed (use-after-free).
            tracing::error!(leftover, "achievement requests leaked at shutdown");
        }
        self.close_worker(WORKER_JOIN_TIMEOUT);
    }
}

#[cfg(test)]
#[path = "request_queue_tests.rs"]
mod tests;
