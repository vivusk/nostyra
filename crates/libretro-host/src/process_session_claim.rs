//! Process-wide guard: libretro cores keep global state, so at most one
//! `CoreSession` may exist per process (the thread-local context alone would
//! allow one per thread).

use std::sync::atomic::{AtomicBool, Ordering};

use crate::host_error::{HostError, HostResult};

/// Cores keep global state, so only one session may exist per process.
static SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Holds the process-wide session slot; released on drop.
pub(crate) struct ProcessSessionClaim;

impl ProcessSessionClaim {
    pub(crate) fn acquire() -> HostResult<Self> {
        SESSION_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| HostError::SessionAlreadyActive)
    }
}

impl Drop for ProcessSessionClaim {
    fn drop(&mut self) {
        SESSION_ACTIVE.store(false, Ordering::Release);
    }
}
