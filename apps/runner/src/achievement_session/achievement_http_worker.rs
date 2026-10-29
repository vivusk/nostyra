//! How the session reaches the server: the real HTTPS transport (User-Agent per
//! session) on the achievements crate's FIFO HTTP worker, or, in debug builds, the
//! canned fake server of `NOSTYRA_RA_FAKE_SERVER=<dir>` (process tests; same worker).
//! Both are wrapped in the offline answer cache (`OfflineCacheTransport`), so a game
//! played online before also starts offline. In-process tests pass their own backend
//! (inline dispatch). Also builds the job that replays the offline journal on that
//! worker at session start.

use std::path::Path;
use std::sync::Arc;

use achievements::{
    replay_pending_unlocks, Credentials, FakeTransport, OfflineCacheState, OfflineCacheTransport,
    PendingJournal, RequestDispatch, ServerTransport, TransportJob, UreqTransport,
};

/// Debug-build env: directory of `<request kind>.json` answers for a fake server.
const FAKE_SERVER_ENV: &str = "NOSTYRA_RA_FAKE_SERVER";

/// Transport + dispatch for one session's client.
pub struct SessionBackend {
    pub transport: Arc<dyn ServerTransport>,
    pub dispatch: RequestDispatch,
    /// What the offline cache served (`None` without the cache: plain tests).
    pub offline: Option<Arc<OfflineCacheState>>,
}

impl SessionBackend {
    /// `transport` as is (no offline cache; in-process tests).
    #[cfg(test)]
    pub fn new(transport: Arc<dyn ServerTransport>, dispatch: RequestDispatch) -> Self {
        Self {
            transport,
            dispatch,
            offline: None,
        }
    }

    /// `transport` behind the offline answer cache of `data_root`.
    pub fn with_offline_cache(
        transport: Arc<dyn ServerTransport>,
        dispatch: RequestDispatch,
        data_root: &Path,
    ) -> Self {
        let cached = OfflineCacheTransport::new(transport, data_root);
        let offline = Some(cached.state());
        Self {
            transport: Arc::new(cached),
            dispatch,
            offline,
        }
    }
}

/// The runner's backend for `user_agent`: worker dispatch over ureq, or the fake
/// server directory from [`FAKE_SERVER_ENV`] in debug builds (process tests: give the
/// runner a temp data dir, the fake's answers land in its offline cache too).
pub fn runner_backend(user_agent: &str, data_root: &Path) -> SessionBackend {
    let fake_dir = cfg!(debug_assertions)
        .then(|| std::env::var_os(FAKE_SERVER_ENV))
        .flatten();
    let transport: Arc<dyn ServerTransport> = match fake_dir {
        Some(dir) => match FakeTransport::from_dir(Path::new(&dir)) {
            Ok(fake) => {
                tracing::warn!(dir = %Path::new(&dir).display(), "achievements use a FAKE server");
                Arc::new(fake)
            }
            Err(err) => {
                tracing::error!(%err, "fake achievement server unreadable; using an empty fake");
                Arc::new(FakeTransport::default())
            }
        },
        None => Arc::new(UreqTransport::new(user_agent)),
    };
    SessionBackend::with_offline_cache(transport, RequestDispatch::Worker, data_root)
}

/// Replays journal leftovers of `credentials.user` (other users' entries stay) on the
/// HTTP worker; the replay removes delivered / refused entries itself.
pub fn replay_journal_job(data_root: &Path, credentials: &Credentials) -> TransportJob {
    let journal = PendingJournal::new(data_root);
    let credentials = credentials.clone();
    Box::new(move |transport| {
        let summary = replay_pending_unlocks(&journal, transport, &credentials);
        if summary != Default::default() {
            tracing::info!(?summary, "offline achievement journal replayed");
        }
    })
}
