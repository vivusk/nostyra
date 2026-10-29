//! `extern "C"` callbacks rc_client calls (read memory, server call, events, results,
//! log). Each finds the [`ClientState`] through `rc_client_get_userdata` and never lets
//! a panic unwind into C (`catch_unwind`, code standards).

use std::ffi::{c_char, c_int, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};

use super::api_ffi::RcApiRequest;
use super::c_text::owned_text;
use super::client_ffi::{rc_client_get_userdata, ServerCallback};
use super::client_types::{RcClient, RcClientEvent};
use super::event_conversion;
use super::server_callback::PendingCallback;
use crate::achievement_events::{ClientCompletion, ClientOp};
use crate::client_state::ClientState;
use crate::server_transport::ServerRequest;

/// Runs `body`, turning a panic into `fallback` (logged).
fn guard<R>(fallback: R, body: impl FnOnce() -> R) -> R {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or_else(|_| {
        tracing::error!("panic inside an rcheevos callback");
        fallback
    })
}

/// # Safety
/// `client` is a live client whose userdata is its boxed `ClientState`
/// (`RawClient::create` sets it before any call that can call back).
unsafe fn state<'a>(client: *const RcClient) -> Option<&'a ClientState> {
    // SAFETY: per the contract above; NULL userdata → None.
    unsafe { (rc_client_get_userdata(client) as *const ClientState).as_ref() }
}

pub unsafe extern "C" fn read_memory(
    address: u32,
    buffer: *mut u8,
    num_bytes: u32,
    client: *mut RcClient,
) -> u32 {
    guard(0, || {
        if buffer.is_null() || num_bytes == 0 {
            return 0;
        }
        // SAFETY: rc_client passes a writable buffer of `num_bytes` bytes and its own
        // live client.
        let (buffer, state) = unsafe {
            (
                std::slice::from_raw_parts_mut(buffer, num_bytes as usize),
                state(client),
            )
        };
        state.map_or(0, |state| state.read_memory(address, buffer))
    })
}

pub unsafe extern "C" fn server_call(
    request: *const RcApiRequest,
    callback: ServerCallback,
    callback_data: *mut c_void,
    client: *mut RcClient,
) {
    let pending = PendingCallback::new(callback, callback_data);
    guard((), move || {
        // SAFETY: request strings are valid for this call (copied here); live client.
        let (request, state) = unsafe {
            let request = &*request;
            let owned = ServerRequest {
                url: owned_text(request.url),
                post_data: owned_text(request.post_data),
                content_type: owned_text(request.content_type),
            };
            (owned, state(client))
        };
        match state {
            Some(state) => state.queue.submit(request, pending),
            // Unreachable by construction (userdata set right after create); a callback
            // that never fires only leaks its request data.
            None => tracing::error!("rcheevos server call without client state"),
        }
    });
}

pub unsafe extern "C" fn on_event(event: *const RcClientEvent, client: *mut RcClient) {
    guard((), || {
        // SAFETY: event and its members are valid during the handler; live client.
        let (event, state) = unsafe { (event_conversion::event(event), state(client)) };
        if let (Some(event), Some(state)) = (event, state) {
            state.push_event(event);
        }
    });
}

pub unsafe extern "C" fn on_client_callback(
    result: c_int,
    error_message: *const c_char,
    client: *mut RcClient,
    userdata: *mut c_void,
) {
    guard((), || {
        // SAFETY: message NULL or NUL-terminated during the call; rc_client suppresses
        // user callbacks after destroy, so `client` is live here.
        let (message, state) = unsafe { (owned_text(error_message), state(client)) };
        if let (Some(op), Some(state)) = (ClientOp::from_tag(userdata as usize), state) {
            state.push_completion(ClientCompletion {
                op,
                result,
                message,
            });
        }
    });
}

pub unsafe extern "C" fn on_log(message: *const c_char, _client: *const RcClient) {
    guard((), || {
        // SAFETY: NUL-terminated message valid during the call.
        let message = unsafe { owned_text(message) };
        tracing::warn!(target: "rcheevos", "{message}");
    });
}
