//! The one achievement HTTP worker thread: FIFO, runs the transport for owned
//! `(id, request)` items and sends `(id, response)` back. It never sees rc_client or a
//! callback, so it can block forever without affecting the client (shutdown detaches it).
//! Jobs (the offline-journal replay) run on the same FIFO; background jobs (badge
//! downloads) wait until no request or job is queued, so they never delay an award.

use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::request_queue::RequestId;
use crate::server_transport::{ServerRequest, ServerResponse, ServerTransport};

/// Work that only needs the transport (no rc_client, no callback), e.g. replaying
/// the offline journal; `Send` so it can run on the HTTP worker.
pub type TransportJob = Box<dyn FnOnce(&dyn ServerTransport) + Send>;

/// Sent to the HTTP worker: owned data only (no callback, no rc_client).
pub(crate) enum WorkItem {
    Request {
        id: RequestId,
        request: ServerRequest,
    },
    Job(TransportJob),
    /// Low priority: runs only while nothing else is queued.
    BackgroundJob(TransportJob),
}

/// Returned by the HTTP worker.
pub(crate) struct WorkResult {
    pub(crate) id: RequestId,
    pub(crate) response: ServerResponse,
}

pub(crate) struct HttpWorker {
    pub(crate) to_worker: Option<Sender<WorkItem>>,
    pub(crate) from_worker: Receiver<WorkResult>,
    pub(crate) handle: Option<JoinHandle<()>>,
}

impl HttpWorker {
    /// Closes the channel and joins for up to `timeout`; a worker stuck in a blocking
    /// call is detached (it holds no callback). Returns true when detached. Idempotent.
    pub(crate) fn close(&mut self, timeout: Duration) -> bool {
        self.to_worker = None;
        let Some(handle) = self.handle.take() else {
            return false;
        };
        let deadline = Instant::now() + timeout;
        while !handle.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if handle.is_finished() {
            let _ = handle.join();
            false
        } else {
            tracing::warn!("achievement HTTP worker still busy at shutdown; detached");
            true
        }
    }
}

pub(crate) fn spawn_worker(transport: Arc<dyn ServerTransport>) -> HttpWorker {
    let (to_worker, work_rx) = channel::<WorkItem>();
    let (result_tx, from_worker) = channel::<WorkResult>();
    let spawned = std::thread::Builder::new()
        .name("achievement-http-worker".into())
        .spawn(move || run_worker(transport.as_ref(), &work_rx, &result_tx));
    let handle = match spawned {
        Ok(handle) => Some(handle),
        Err(err) => {
            tracing::error!(%err, "cannot start achievement HTTP worker; requests fail offline");
            None
        }
    };
    HttpWorker {
        to_worker: handle.as_ref().map(|_| to_worker),
        from_worker,
        handle,
    }
}

/// Worker loop: requests and jobs in arrival order; background jobs only once the
/// channel is empty (one at a time, so a newly queued request waits at most one GET).
/// Ends when the client closes the channel (queued background jobs are dropped).
fn run_worker(
    transport: &dyn ServerTransport,
    work_rx: &Receiver<WorkItem>,
    result_tx: &Sender<WorkResult>,
) {
    let mut background: VecDeque<TransportJob> = VecDeque::new();
    loop {
        let item = match work_rx.try_recv() {
            Ok(item) => item,
            Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => match background.pop_front() {
                Some(job) => {
                    job(transport);
                    continue;
                }
                None => match work_rx.recv() {
                    Ok(item) => item,
                    Err(_) => return,
                },
            },
        };
        match item {
            WorkItem::Request { id, request } => {
                let response = transport.call(&request);
                if result_tx.send(WorkResult { id, response }).is_err() {
                    return; // client gone: late results are discarded
                }
            }
            WorkItem::Job(job) => job(transport),
            WorkItem::BackgroundJob(job) => background.push_back(job),
        }
    }
}
