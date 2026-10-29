//! The `(callback, callback_data)` pair rc_client hands to `server_call`. It holds raw
//! pointers, so it is `!Send` by construction: it can only live in the client-thread
//! registry and fire on that thread. `fire` consumes it, so it fires at most once.

use std::ffi::c_void;

use super::client_ffi::ServerCallback;
use crate::server_transport::ServerResponse;

/// One in-flight request's completion callback.
pub struct PendingCallback {
    callback: ServerCallback,
    data: *mut c_void,
}

impl PendingCallback {
    /// Wraps the pair received in the `server_call` trampoline.
    pub(super) fn new(callback: ServerCallback, data: *mut c_void) -> Self {
        Self { callback, data }
    }

    /// Delivers `response` to rc_client. Must run on the client thread while the
    /// client is alive: `AchievementClient` drains every pending callback before
    /// `rc_client_destroy` (D4), and this type cannot leave the client thread.
    pub fn fire(self, response: &ServerResponse) {
        super::api_calls::with_c_response(response, |server| {
            // SAFETY: callback/data came from rc_client's server_call for a live client
            // (see above); the response and its NUL-terminated body outlive the call;
            // rc_client copies what it keeps.
            unsafe { (self.callback)(server, self.data) }
        });
    }
}

/// Test-only callbacks that count how often (and with which status) they fired.
#[cfg(test)]
pub mod test_callbacks {
    use super::super::api_ffi::RcApiServerResponse;
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        /// (tag, status, body) of every test callback fired on this thread.
        pub static FIRED: RefCell<Vec<(usize, i32, String)>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "C" fn record(response: *const RcApiServerResponse, data: *mut c_void) {
        // SAFETY: `fire` passes a valid response whose body is NUL-terminated.
        let (status, body) = unsafe {
            let response = &*response;
            (
                response.http_status_code,
                super::super::c_text::owned_text(response.body),
            )
        };
        FIRED.with(|fired| fired.borrow_mut().push((data as usize, status, body)));
    }

    /// A pending callback that records `tag` into [`FIRED`] when fired.
    pub fn recording(tag: usize) -> PendingCallback {
        PendingCallback::new(record, tag as *mut c_void)
    }
}
