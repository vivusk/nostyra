//! rcheevos v12.5.0 `rc_api_*` declarations (`include/rc_api_request.h`,
//! `rc_api_user.h`, `rc_api_runtime.h`, `rc_util.h`, `rc_error.h`): the request
//! builders and response parsers the launcher login and the offline journal replay use
//! without an `rc_client`.

use std::ffi::{c_char, c_int};

/// `rc_buffer_chunk_t`.
#[repr(C)]
pub struct RcBufferChunk {
    pub write: *mut u8,
    pub end: *mut u8,
    pub start: *mut u8,
    pub next: *mut RcBufferChunk,
}

/// `rc_buffer_t`: chunk + 256 inline bytes. Self-referential after init, so a struct
/// holding one is boxed and never moved between `init_*` and `destroy_*`.
#[repr(C)]
pub struct RcBuffer {
    pub chunk: RcBufferChunk,
    pub data: [u8; 256],
}

/// `rc_api_request_t` (also what `rc_client` hands to `server_call`).
#[repr(C)]
pub struct RcApiRequest {
    pub url: *const c_char,
    pub post_data: *const c_char,
    pub content_type: *const c_char,
    pub buffer: RcBuffer,
}

/// `rc_api_server_response_t`.
#[repr(C)]
pub struct RcApiServerResponse {
    pub body: *const c_char,
    pub body_length: usize,
    pub http_status_code: c_int,
}

/// `rc_api_response_t`.
#[repr(C)]
pub struct RcApiResponse {
    pub succeeded: c_int,
    pub error_message: *const c_char,
    pub error_code: *const c_char,
    pub buffer: RcBuffer,
}

/// `rc_api_login_request_t`.
#[repr(C)]
pub struct RcApiLoginRequest {
    pub username: *const c_char,
    pub api_token: *const c_char,
    pub password: *const c_char,
}

/// `rc_api_login_response_t`.
#[repr(C)]
pub struct RcApiLoginResponse {
    pub username: *const c_char,
    pub api_token: *const c_char,
    pub score: u32,
    pub score_softcore: u32,
    pub num_unread_messages: u32,
    pub display_name: *const c_char,
    pub avatar_url: *const c_char,
    pub avatar_last_updated: i64,
    pub response: RcApiResponse,
}

/// `rc_api_award_achievement_request_t`.
#[repr(C)]
pub struct RcApiAwardRequest {
    pub username: *const c_char,
    pub api_token: *const c_char,
    pub achievement_id: u32,
    pub hardcore: u32,
    pub game_hash: *const c_char,
    pub seconds_since_unlock: u32,
}

/// `rc_api_award_achievement_response_t`.
#[repr(C)]
pub struct RcApiAwardResponse {
    pub awarded_achievement_id: u32,
    pub new_player_score: u32,
    pub new_player_score_softcore: u32,
    pub achievements_remaining: u32,
    pub response: RcApiResponse,
}

/// `rc_api_submit_lboard_entry_request_t`.
#[repr(C)]
pub struct RcApiSubmitRequest {
    pub username: *const c_char,
    pub api_token: *const c_char,
    pub leaderboard_id: u32,
    pub score: i32,
    pub game_hash: *const c_char,
    pub seconds_since_completion: u32,
}

/// `rc_api_lboard_entry_t`.
#[repr(C)]
pub struct RcApiLboardEntry {
    pub username: *const c_char,
    pub rank: u32,
    pub score: i32,
}

/// `rc_api_submit_lboard_entry_response_t`.
#[repr(C)]
pub struct RcApiSubmitResponse {
    pub submitted_score: i32,
    pub best_score: i32,
    pub new_rank: u32,
    pub num_entries: u32,
    pub top_entries: *const RcApiLboardEntry,
    pub num_top_entries: u32,
    pub response: RcApiResponse,
}

unsafe extern "C" {
    pub fn rc_api_init_login_request(req: *mut RcApiRequest, p: *const RcApiLoginRequest) -> c_int;
    pub fn rc_api_process_login_server_response(
        resp: *mut RcApiLoginResponse,
        server: *const RcApiServerResponse,
    ) -> c_int;
    pub fn rc_api_destroy_login_response(resp: *mut RcApiLoginResponse);

    pub fn rc_api_init_award_achievement_request(
        req: *mut RcApiRequest,
        p: *const RcApiAwardRequest,
    ) -> c_int;
    pub fn rc_api_process_award_achievement_server_response(
        resp: *mut RcApiAwardResponse,
        server: *const RcApiServerResponse,
    ) -> c_int;
    pub fn rc_api_destroy_award_achievement_response(resp: *mut RcApiAwardResponse);

    pub fn rc_api_init_submit_lboard_entry_request(
        req: *mut RcApiRequest,
        p: *const RcApiSubmitRequest,
    ) -> c_int;
    pub fn rc_api_process_submit_lboard_entry_server_response(
        resp: *mut RcApiSubmitResponse,
        server: *const RcApiServerResponse,
    ) -> c_int;
    pub fn rc_api_destroy_submit_lboard_entry_response(resp: *mut RcApiSubmitResponse);

    pub fn rc_api_destroy_request(req: *mut RcApiRequest);

    /// Static English text for an `RC_*` result code (`rc_error.h`).
    pub fn rc_error_str(ret: c_int) -> *const c_char;
}
