//! rcheevos v12.5.0 `rc_client_*` declarations (`include/rc_client.h`) and callback
//! types. Only the functions Nostyra calls (now or in phases 02–05) are declared.

use std::ffi::{c_char, c_int, c_void};

use super::api_ffi::{RcApiRequest, RcApiServerResponse};
use super::client_types::*;
use super::hash_ffi::RcHashCallbacks;

pub type ServerCallback =
    unsafe extern "C" fn(response: *const RcApiServerResponse, callback_data: *mut c_void);
pub type ReadMemoryFn = unsafe extern "C" fn(
    address: u32,
    buffer: *mut u8,
    num_bytes: u32,
    client: *mut RcClient,
) -> u32;
pub type ServerCallFn = unsafe extern "C" fn(
    request: *const RcApiRequest,
    callback: ServerCallback,
    callback_data: *mut c_void,
    client: *mut RcClient,
);
pub type ClientCallback = unsafe extern "C" fn(
    result: c_int,
    error_message: *const c_char,
    client: *mut RcClient,
    userdata: *mut c_void,
);
pub type MessageCallback = unsafe extern "C" fn(message: *const c_char, client: *const RcClient);
pub type EventHandler = unsafe extern "C" fn(event: *const RcClientEvent, client: *mut RcClient);

/// `RC_CLIENT_LOG_LEVEL_WARN`: errors + warnings; never request bodies.
pub const LOG_LEVEL_WARN: c_int = 2;

/// `RC_CLIENT_EVENT_*` values (`rc_client.h`).
pub mod event_type {
    pub const ACHIEVEMENT_TRIGGERED: u32 = 1;
    pub const LEADERBOARD_STARTED: u32 = 2;
    pub const LEADERBOARD_FAILED: u32 = 3;
    pub const LEADERBOARD_SUBMITTED: u32 = 4;
    pub const CHALLENGE_INDICATOR_SHOW: u32 = 5;
    pub const CHALLENGE_INDICATOR_HIDE: u32 = 6;
    pub const PROGRESS_INDICATOR_SHOW: u32 = 7;
    pub const PROGRESS_INDICATOR_HIDE: u32 = 8;
    pub const PROGRESS_INDICATOR_UPDATE: u32 = 9;
    pub const LEADERBOARD_TRACKER_SHOW: u32 = 10;
    pub const LEADERBOARD_TRACKER_HIDE: u32 = 11;
    pub const LEADERBOARD_TRACKER_UPDATE: u32 = 12;
    pub const LEADERBOARD_SCOREBOARD: u32 = 13;
    pub const RESET: u32 = 14;
    pub const GAME_COMPLETED: u32 = 15;
    pub const SERVER_ERROR: u32 = 16;
    pub const DISCONNECTED: u32 = 17;
    pub const RECONNECTED: u32 = 18;
    pub const SUBSET_COMPLETED: u32 = 19;
}

unsafe extern "C" {
    pub fn rc_client_create(read_memory: ReadMemoryFn, server_call: ServerCallFn) -> *mut RcClient;
    pub fn rc_client_destroy(client: *mut RcClient);
    pub fn rc_client_set_userdata(client: *mut RcClient, userdata: *mut c_void);
    pub fn rc_client_get_userdata(client: *const RcClient) -> *mut c_void;
    pub fn rc_client_enable_logging(
        client: *mut RcClient,
        level: c_int,
        callback: Option<MessageCallback>,
    );
    pub fn rc_client_set_event_handler(client: *mut RcClient, handler: Option<EventHandler>);
    pub fn rc_client_set_hardcore_enabled(client: *mut RcClient, enabled: c_int);
    pub fn rc_client_get_hardcore_enabled(client: *const RcClient) -> c_int;
    pub fn rc_client_get_user_agent_clause(
        client: *mut RcClient,
        buffer: *mut c_char,
        buffer_size: usize,
    ) -> usize;

    pub fn rc_client_begin_login_with_token(
        client: *mut RcClient,
        username: *const c_char,
        token: *const c_char,
        callback: ClientCallback,
        userdata: *mut c_void,
    ) -> *mut c_void;
    pub fn rc_client_get_user_info(client: *const RcClient) -> *const RcUser;

    pub fn rc_client_begin_identify_and_load_game(
        client: *mut RcClient,
        console_id: u32,
        file_path: *const c_char,
        data: *const u8,
        data_size: usize,
        callback: ClientCallback,
        userdata: *mut c_void,
    ) -> *mut c_void;
    /// Phase 02 installs the Rust file + CD readers through this.
    #[allow(dead_code)]
    pub fn rc_client_set_hash_callbacks(client: *mut RcClient, callbacks: *const RcHashCallbacks);
    pub fn rc_client_begin_load_game(
        client: *mut RcClient,
        hash: *const c_char,
        callback: ClientCallback,
        userdata: *mut c_void,
    ) -> *mut c_void;
    pub fn rc_client_begin_change_media(
        client: *mut RcClient,
        hash: *const c_char,
        callback: ClientCallback,
        userdata: *mut c_void,
    ) -> *mut c_void;
    pub fn rc_client_get_load_game_state(client: *const RcClient) -> c_int;
    pub fn rc_client_is_game_loaded(client: *const RcClient) -> c_int;
    pub fn rc_client_unload_game(client: *mut RcClient);
    pub fn rc_client_get_game_info(client: *const RcClient) -> *const RcGame;
    pub fn rc_client_get_user_game_summary(
        client: *const RcClient,
        summary: *mut RcUserGameSummary,
    );

    pub fn rc_client_create_achievement_list(
        client: *mut RcClient,
        category: c_int,
        grouping: c_int,
    ) -> *mut RcAchievementList;
    pub fn rc_client_destroy_achievement_list(list: *mut RcAchievementList);
    pub fn rc_client_create_leaderboard_list(
        client: *mut RcClient,
        grouping: c_int,
    ) -> *mut RcLeaderboardList;
    pub fn rc_client_destroy_leaderboard_list(list: *mut RcLeaderboardList);

    pub fn rc_client_do_frame(client: *mut RcClient);
    pub fn rc_client_idle(client: *mut RcClient);
    pub fn rc_client_reset(client: *mut RcClient);
    pub fn rc_client_can_pause(client: *mut RcClient, frames_remaining: *mut u32) -> c_int;
    pub fn rc_client_progress_size(client: *mut RcClient) -> usize;
    pub fn rc_client_serialize_progress_sized(
        client: *mut RcClient,
        buffer: *mut u8,
        buffer_size: usize,
    ) -> c_int;
    pub fn rc_client_deserialize_progress_sized(
        client: *mut RcClient,
        serialized: *const u8,
        serialized_size: usize,
    ) -> c_int;
}
