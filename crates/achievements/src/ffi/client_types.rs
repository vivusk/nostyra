//! `#[repr(C)]` mirrors of the rcheevos v12.5.0 `include/rc_client.h` structs Nostyra
//! reads. Full structs (not prefixes) so `ffi_layout_tests` can compare every size and
//! offset with the C compiler's view (`layout_probe.c`). `time_t` is 64-bit on every
//! supported target (MSVC x64, glibc x86_64/aarch64, macOS).

use std::ffi::{c_char, c_int};

/// Opaque `rc_client_t`.
#[repr(C)]
pub struct RcClient {
    _private: [u8; 0],
}

/// `RC_CLIENT_LEADERBOARD_DISPLAY_SIZE`.
pub const LEADERBOARD_DISPLAY_SIZE: usize = 24;

/// `rc_client_user_t`.
#[repr(C)]
pub struct RcUser {
    pub display_name: *const c_char,
    pub username: *const c_char,
    pub token: *const c_char,
    pub score: u32,
    pub score_softcore: u32,
    pub num_unread_messages: u32,
    pub avatar_url: *const c_char,
    pub avatar_last_updated: i64,
}

/// `rc_client_user_game_summary_t`.
#[repr(C)]
#[derive(Default)]
pub struct RcUserGameSummary {
    pub num_core_achievements: u32,
    pub num_unofficial_achievements: u32,
    pub num_unlocked_achievements: u32,
    pub num_unsupported_achievements: u32,
    pub points_core: u32,
    pub points_unlocked: u32,
    pub beaten_time: i64,
    pub completed_time: i64,
}

/// `rc_client_game_t`.
#[repr(C)]
pub struct RcGame {
    pub id: u32,
    pub console_id: u32,
    pub title: *const c_char,
    pub hash: *const c_char,
    pub badge_name: *const c_char,
    pub badge_url: *const c_char,
}

/// `rc_client_subset_t`.
#[repr(C)]
pub struct RcSubset {
    pub id: u32,
    pub title: *const c_char,
    pub badge_name: [c_char; 16],
    pub num_achievements: u32,
    pub num_leaderboards: u32,
    pub badge_url: *const c_char,
}

/// `rc_client_achievement_t`.
#[repr(C)]
pub struct RcAchievement {
    pub title: *const c_char,
    pub description: *const c_char,
    pub badge_name: [c_char; 8],
    pub measured_progress: [c_char; 24],
    pub measured_percent: f32,
    pub id: u32,
    pub points: u32,
    pub unlock_time: i64,
    pub state: u8,
    pub category: u8,
    pub bucket: u8,
    pub unlocked: u8,
    pub rarity: f32,
    pub rarity_hardcore: f32,
    pub kind: u8,
    pub badge_url: *const c_char,
    pub badge_locked_url: *const c_char,
}

/// `rc_client_achievement_bucket_t`.
#[repr(C)]
pub struct RcAchievementBucket {
    pub achievements: *const *const RcAchievement,
    pub num_achievements: u32,
    pub label: *const c_char,
    pub subset_id: u32,
    pub bucket_type: u8,
}

/// `rc_client_achievement_list_t`.
#[repr(C)]
pub struct RcAchievementList {
    pub buckets: *const RcAchievementBucket,
    pub num_buckets: u32,
}

/// `rc_client_leaderboard_t`.
#[repr(C)]
pub struct RcLeaderboard {
    pub title: *const c_char,
    pub description: *const c_char,
    pub tracker_value: *const c_char,
    pub id: u32,
    pub state: u8,
    pub format: u8,
    pub lower_is_better: u8,
}

/// `rc_client_leaderboard_tracker_t`.
#[repr(C)]
pub struct RcLeaderboardTracker {
    pub display: [c_char; LEADERBOARD_DISPLAY_SIZE],
    pub id: u32,
}

/// `rc_client_leaderboard_bucket_t`.
#[repr(C)]
pub struct RcLeaderboardBucket {
    pub leaderboards: *const *const RcLeaderboard,
    pub num_leaderboards: u32,
    pub label: *const c_char,
    pub subset_id: u32,
    pub bucket_type: u8,
}

/// `rc_client_leaderboard_list_t`.
#[repr(C)]
pub struct RcLeaderboardList {
    pub buckets: *const RcLeaderboardBucket,
    pub num_buckets: u32,
}

/// `rc_client_leaderboard_scoreboard_entry_t`.
#[repr(C)]
pub struct RcScoreboardEntry {
    pub username: *const c_char,
    pub rank: u32,
    pub score: [c_char; LEADERBOARD_DISPLAY_SIZE],
}

/// `rc_client_leaderboard_scoreboard_t`.
#[repr(C)]
pub struct RcScoreboard {
    pub leaderboard_id: u32,
    pub submitted_score: [c_char; LEADERBOARD_DISPLAY_SIZE],
    pub best_score: [c_char; LEADERBOARD_DISPLAY_SIZE],
    pub new_rank: u32,
    pub num_entries: u32,
    pub top_entries: *const RcScoreboardEntry,
    pub num_top_entries: u32,
}

/// `rc_client_server_error_t`.
#[repr(C)]
pub struct RcServerError {
    pub error_message: *const c_char,
    pub api: *const c_char,
    pub result: c_int,
    pub related_id: u32,
}

/// `rc_client_event_t`.
#[repr(C)]
pub struct RcClientEvent {
    pub kind: u32,
    pub achievement: *const RcAchievement,
    pub leaderboard: *const RcLeaderboard,
    pub leaderboard_tracker: *const RcLeaderboardTracker,
    pub leaderboard_scoreboard: *const RcScoreboard,
    pub server_error: *const RcServerError,
    pub subset: *const RcSubset,
}
