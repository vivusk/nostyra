//! RetroAchievements for Nostyra: the vendored rcheevos v12.5.0 (`third-party/rcheevos`,
//! MIT, compiled by build.rs) behind a safe API.
//!
//! - Runner: [`AchievementClient`] (one `rc_client`, single thread) with a
//!   [`ServerTransport`] run inline or on an HTTP worker ([`RequestDispatch`]).
//! - Launcher: [`login_with_password`] and [`replay_pending_unlocks`] use the
//!   client-free `rc_api` builders.
//! - Shared files: credentials, offline unlock journal, game snapshot, badge cache,
//!   offline answer cache ([`OfflineCacheTransport`]: sessions start offline).
//! - Game identity: [`hash_request`] / [`content_hash`] (Rust file reader +
//!   `disc-image` cdreader, installed on every client too).
//! - Console memory: [`MemoryRegions`] / [`LazyMemoryRegions`] (`rc_libretro` over a
//!   core's memory map or SAVE_RAM / SYSTEM_RAM).
//! - Hardcore core-option rules: [`is_option_disallowed`].
//!
//! `unsafe` lives only in the private `ffi` module (denied everywhere else).

#![deny(unsafe_code)]

mod achievement_client;
mod achievement_client_shutdown;
mod achievement_events;
mod achievement_info;
mod badge_paths;
mod client_state;
mod content_hash;
mod credentials_file;
mod fake_game_set_json;
mod fake_server_dir;
mod fake_server_transport;
#[allow(unsafe_code)] // the rcheevos FFI boundary (code standards)
mod ffi;
mod game_snapshot;
mod hardcore_flag;
mod hardcore_rules;
mod lazy_memory_regions;
mod memory_regions;
mod offline_cache_file_names;
mod offline_cache_state;
mod offline_cache_transport;
mod offline_response_cache;
mod password_login;
mod pending_unlock_journal;
mod pending_unlock_replay;
mod request_http_worker;
mod request_observer;
mod request_queue;
mod server_transport;
mod user_agent;

pub use achievement_client::{AchievementClient, ClientCreateError};
pub use achievement_client_shutdown::ShutdownReport;
pub use achievement_events::{
    AchievementEvent, ClientCompletion, ClientOp, RC_ACCESS_DENIED, RC_EXPIRED_TOKEN,
    RC_HARDCORE_DISABLED, RC_INVALID_CREDENTIALS, RC_LOGIN_REQUIRED, RC_NO_GAME_LOADED,
    RC_NO_RESPONSE, RC_OK,
};
pub use achievement_info::{
    AchievementBucket, AchievementInfo, GameInfo, GameSummary, LeaderboardBucket, LeaderboardInfo,
    LoadGameState, Scoreboard, ScoreboardEntry, UserInfo,
};
pub use badge_paths::{badge_cache_dir, badge_path, badge_url, game_icon_path, game_icon_url};
pub use client_state::MemoryReader;
pub use content_hash::{content_hash, hash_request, HashRequest, HashUnavailable};
pub use credentials_file::{
    credentials_from_env, credentials_path, delete_credentials, load_credentials,
    resolve_credentials, save_credentials, Credentials, TOKEN_ENV, USER_ENV,
};
pub use fake_game_set_json::fake_game_set_json;
pub use fake_server_dir::FAKE_GAME_HASH_FILE;
pub use fake_server_transport::{
    FakeReply, FakeTransport, FAKE_LOGIN_JSON, FAKE_UNKNOWN_GAME_JSON,
};
pub use ffi::error_text;
pub use game_snapshot::{
    load_game_snapshot, save_game_snapshot, snapshot_path, GameSnapshot, SnapshotAchievement,
    SNAPSHOT_SCHEMA,
};
pub use hardcore_flag::{hardcore_ui_enabled, HARDCORE_UI_ENABLED};
pub use hardcore_rules::is_option_disallowed;
pub use lazy_memory_regions::{LazyMemoryRegions, RegionStatus, MAX_EMPTY_FRAMES};
pub use memory_regions::{CoreMemoryLookup, MemoryRegions};
pub use offline_cache_state::OfflineCacheState;
pub use offline_cache_transport::{OfflineCacheTransport, OFFLINE_NO_CACHE_BODY};
pub use password_login::{login_with_password, LoginError, LoginSuccess, Password};
pub use pending_unlock_journal::{PendingEntry, PendingJournal, PendingKind};
pub use pending_unlock_replay::{replay_pending_unlocks, ReplayOutcome, ReplaySummary};
pub use request_http_worker::TransportJob;
pub use request_observer::{RequestObservation, TrackedSubmission};
pub use request_queue::RequestDispatch;
pub use server_transport::{
    form_field, is_retryable_response, request_kind, ServerRequest, ServerResponse,
    ServerTransport, UreqTransport, BODY_OFFLINE, BODY_SHUTDOWN, BODY_TIMEOUT, STATUS_NO_RESPONSE,
};
pub use user_agent::{rcheevos_user_agent_clause, user_agent};
