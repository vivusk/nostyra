//! In-memory provider for tests, with scripted faults: offline, the Nth call
//! failing, errors on matching calls (rev mismatch once, rate limit, quota full),
//! corrupt / missing blobs, no conditional writes (the Google Drive / desktop-client
//! model) and a hook run before every call (e.g. another PC commits mid-push).
//! The trait calls themselves are in `fake_cloud_provider_ops.rs`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::cloud_error::CloudError;

/// Which trait call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeOp {
    List,
    Read,
    Write,
    WriteLarge,
    Delete,
    DeleteDir,
    /// Between two chunks of a streamed read (`read_to`): hook only, never logged
    /// or faulted.
    ReadChunk,
}

/// Runs before each call with (op, path); may call the provider itself.
pub type FakeHook = Arc<dyn Fn(FakeOp, &str) + Send + Sync>;

#[derive(Debug)]
pub(super) struct Fault {
    pub(super) op: Option<FakeOp>,
    pub(super) path_contains: String,
    pub(super) error: CloudError,
    pub(super) remaining: usize,
}

#[derive(Debug)]
pub(super) struct State {
    pub(super) files: BTreeMap<String, (Vec<u8>, u64)>,
    pub(super) next_rev: u64,
    pub(super) calls: Vec<(FakeOp, String)>,
    pub(super) faults: Vec<Fault>,
    /// (calls left before it fires, error).
    pub(super) nth: Option<(usize, CloudError)>,
    pub(super) offline: bool,
    pub(super) conditional: bool,
}

/// The fake storage (share it behind an `Arc` between simulated PCs).
pub struct FakeCloudProvider {
    pub(super) state: Mutex<State>,
    pub(super) hook: Mutex<Option<FakeHook>>,
}

impl Default for FakeCloudProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeCloudProvider {
    /// Empty storage with conditional writes.
    pub fn new() -> Self {
        let state = State {
            files: BTreeMap::new(),
            next_rev: 1,
            calls: Vec::new(),
            faults: Vec::new(),
            nth: None,
            offline: false,
            conditional: true,
        };
        Self {
            state: Mutex::new(state),
            hook: Mutex::new(None),
        }
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// `false` = `expect` revs are ignored (last writer wins).
    pub fn set_conditional_writes(&self, on: bool) {
        self.lock().conditional = on;
    }

    /// Every call fails with `Offline` while set.
    pub fn set_offline(&self, offline: bool) {
        self.lock().offline = offline;
    }

    /// The `n`-th call from now (1-based) fails with `error`.
    pub fn fail_nth_call(&self, n: usize, error: CloudError) {
        self.lock().nth = Some((n.max(1), error));
    }

    /// The next `times` calls of `op` (any op: `None`) whose path contains
    /// `path_contains` fail with `error`.
    pub fn fail_next(
        &self,
        op: Option<FakeOp>,
        path_contains: &str,
        error: CloudError,
        times: usize,
    ) {
        self.lock().faults.push(Fault {
            op,
            path_contains: path_contains.to_string(),
            error,
            remaining: times,
        });
    }

    /// The next write to a path containing `path_contains` loses its rev check.
    pub fn rev_mismatch_once(&self, path_contains: &str) {
        self.fail_next(
            Some(FakeOp::Write),
            path_contains,
            CloudError::RevMismatch,
            1,
        );
    }

    /// Installs (or clears) the hook run before each call.
    pub fn set_hook(&self, hook: Option<FakeHook>) {
        *self.hook.lock().unwrap_or_else(|p| p.into_inner()) = hook;
    }

    /// Flips the stored bytes of `path` (a corrupt blob). False when absent.
    pub fn corrupt(&self, path: &str) -> bool {
        let mut state = self.lock();
        match state.files.get_mut(path) {
            Some((bytes, _)) => {
                bytes.push(0xFF);
                true
            }
            None => false,
        }
    }

    /// Stores `bytes` at `path` directly (no call logged; a new rev).
    pub fn put(&self, path: &str, bytes: &[u8]) {
        let mut state = self.lock();
        let rev = state.next_rev;
        state.next_rev += 1;
        state.files.insert(path.to_string(), (bytes.to_vec(), rev));
    }

    /// Removes `path` or every file below `path/` directly (no call logged).
    pub fn remove(&self, path: &str) {
        let prefix = format!("{path}/");
        self.lock()
            .files
            .retain(|key, _| key != path && !key.starts_with(&prefix));
    }

    /// The stored bytes of `path`.
    pub fn get(&self, path: &str) -> Option<Vec<u8>> {
        self.lock().files.get(path).map(|(bytes, _)| bytes.clone())
    }

    /// Every stored path.
    pub fn paths(&self) -> Vec<String> {
        self.lock().files.keys().cloned().collect()
    }

    /// Calls made so far (op, path), oldest first.
    pub fn calls(&self) -> Vec<(FakeOp, String)> {
        self.lock().calls.clone()
    }

    /// Forgets the call log.
    pub fn clear_calls(&self) {
        self.lock().calls.clear();
    }
}
