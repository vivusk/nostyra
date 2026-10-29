//! Safe wrappers over the client-free `rc_api_*` calls: the shared request builder /
//! response plumbing and the launcher's password login (journal replays use these in
//! `api_replay_calls`). Requests and responses are boxed in place (`rc_buffer_t` is
//! self-referential) and destroyed exactly once.

use std::ffi::c_int;
use std::mem::MaybeUninit;

use super::api_ffi::*;
use super::c_text::{c_string, owned_text, wipe_c_string};
use crate::server_transport::{ServerRequest, ServerResponse};

/// A request the server (or the parser) rejected: `RC_*` code + readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiFailure {
    pub code: i32,
    pub message: String,
}

/// Parsed `login2` success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginReply {
    pub username: String,
    pub display_name: String,
    pub token: String,
    pub score: u32,
    pub score_softcore: u32,
}

/// English text for an `RC_*` code.
pub fn error_text(code: i32) -> String {
    // SAFETY: rc_error_str returns a static NUL-terminated string for any code.
    unsafe { owned_text(rc_error_str(code)) }
}

/// Runs an `rc_api_init_*` builder on a boxed request, copies it out, wipes the C copy
/// of the body (it holds the token or password) and destroys the request.
pub(super) fn build(init: impl FnOnce(*mut RcApiRequest) -> c_int) -> Result<ServerRequest, i32> {
    let mut request: Box<MaybeUninit<RcApiRequest>> = Box::new(MaybeUninit::zeroed());
    let rc = init(request.as_mut_ptr());
    // SAFETY: zeroed is a valid "empty" request; on success the builder filled it. The
    // post body lives in the request's own buffer, so wiping it before destroy is ours
    // to do. Destroyed once, never moved (boxed).
    unsafe {
        let r = request.assume_init_mut();
        let built = ServerRequest {
            url: owned_text(r.url),
            post_data: owned_text(r.post_data),
            content_type: owned_text(r.content_type),
        };
        if !r.post_data.is_null() {
            let len = built.post_data.len();
            std::ptr::write_bytes(r.post_data as *mut u8, 0, len);
        }
        rc_api_destroy_request(r);
        if rc == 0 {
            Ok(built)
        } else {
            Err(rc)
        }
    }
}

/// Lends `response` to C as a NUL-terminated `rc_api_server_response_t`.
pub(super) fn with_c_response<R>(
    response: &ServerResponse,
    f: impl FnOnce(&RcApiServerResponse) -> R,
) -> R {
    let mut body = Vec::with_capacity(response.body.len() + 1);
    body.extend_from_slice(response.body.as_bytes());
    body.push(0);
    let server = RcApiServerResponse {
        body: body.as_ptr().cast(),
        body_length: response.body.len(),
        http_status_code: response.status,
    };
    f(&server)
}

/// Converts a parser result + `rc_api_response_t` into `Ok` / [`ApiFailure`].
pub(super) fn outcome(rc: c_int, response: &RcApiResponse) -> Result<(), ApiFailure> {
    if rc == 0 && response.succeeded != 0 {
        return Ok(());
    }
    let code = if rc == 0 { -1 } else { rc };
    // SAFETY: error_message is NULL or a string in the response buffer, alive here.
    let message = unsafe { owned_text(response.error_message) };
    Err(ApiFailure {
        code,
        message: if message.is_empty() {
            error_text(code)
        } else {
            message
        },
    })
}

/// `r=login2` with a password (launcher login) or a token.
pub fn login_request(
    username: &str,
    password: Option<&str>,
    token: Option<&str>,
) -> Result<ServerRequest, i32> {
    let user = c_string(username);
    let password = password.map(c_string);
    let token = token.map(c_string);
    let params = RcApiLoginRequest {
        username: user.as_ptr(),
        api_token: token.as_ref().map_or(std::ptr::null(), |t| t.as_ptr()),
        password: password.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
    };
    // SAFETY: params and their strings outlive the call.
    let built = build(|r| unsafe { rc_api_init_login_request(r, &params) });
    password.into_iter().chain(token).for_each(wipe_c_string);
    built
}

/// Parses a `login2` answer (error codes −34 invalid credentials, −35 expired token).
pub fn parse_login_response(response: &ServerResponse) -> Result<LoginReply, ApiFailure> {
    let mut parsed: Box<MaybeUninit<RcApiLoginResponse>> = Box::new(MaybeUninit::zeroed());
    // SAFETY: the parser memsets + fills the boxed response; read, then destroyed once.
    unsafe {
        let rc = with_c_response(response, |server| {
            rc_api_process_login_server_response(parsed.as_mut_ptr(), server)
        });
        let p = parsed.assume_init_mut();
        let result = outcome(rc, &p.response).map(|()| LoginReply {
            username: owned_text(p.username),
            display_name: owned_text(p.display_name),
            token: owned_text(p.api_token),
            score: p.score,
            score_softcore: p.score_softcore,
        });
        rc_api_destroy_login_response(p);
        result
    }
}
