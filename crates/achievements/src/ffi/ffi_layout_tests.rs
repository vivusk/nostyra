//! Compares every hand-written `#[repr(C)]` mirror with the C compiler's own
//! `sizeof` / `offsetof` (`layout_probe.c`, built by build.rs with the library's
//! defines), so layout drift on any platform fails here instead of corrupting memory.

use std::ffi::{c_char, CString};
use std::mem::{offset_of, size_of};

use super::api_ffi::*;
use super::client_types::*;
use super::hash_ffi::*;
use super::libretro_ffi::*;

unsafe extern "C" {
    fn nostyra_rc_layout(key: *const c_char) -> usize;
}

fn c_layout(key: &str) -> usize {
    let key = CString::new(key).unwrap_or_default();
    // SAFETY: NUL-terminated key valid for the call; the probe only reads it.
    let value = unsafe { nostyra_rc_layout(key.as_ptr()) };
    assert_ne!(value, usize::MAX, "layout probe does not know {key:?}");
    value
}

macro_rules! size {
    ($rust:ty, $c:literal) => {
        assert_eq!(size_of::<$rust>(), c_layout($c), "sizeof {}", $c);
    };
}

macro_rules! offset {
    ($rust:ty, $field:ident, $c:literal) => {
        assert_eq!(offset_of!($rust, $field), c_layout($c), "offsetof {}", $c);
    };
}

#[test]
fn client_struct_layouts_match_c() {
    size!(RcUser, "rc_client_user_t");
    offset!(
        RcUser,
        avatar_last_updated,
        "rc_client_user_t.avatar_last_updated"
    );
    size!(RcUserGameSummary, "rc_client_user_game_summary_t");
    offset!(
        RcUserGameSummary,
        completed_time,
        "rc_client_user_game_summary_t.completed_time"
    );
    size!(RcGame, "rc_client_game_t");
    offset!(RcGame, badge_url, "rc_client_game_t.badge_url");
    size!(RcSubset, "rc_client_subset_t");
    offset!(RcSubset, badge_url, "rc_client_subset_t.badge_url");
    size!(RcAchievement, "rc_client_achievement_t");
    offset!(
        RcAchievement,
        measured_percent,
        "rc_client_achievement_t.measured_percent"
    );
    offset!(
        RcAchievement,
        unlock_time,
        "rc_client_achievement_t.unlock_time"
    );
    offset!(RcAchievement, unlocked, "rc_client_achievement_t.unlocked");
    offset!(RcAchievement, kind, "rc_client_achievement_t.type");
    offset!(
        RcAchievement,
        badge_locked_url,
        "rc_client_achievement_t.badge_locked_url"
    );
    size!(RcAchievementBucket, "rc_client_achievement_bucket_t");
    offset!(
        RcAchievementBucket,
        bucket_type,
        "rc_client_achievement_bucket_t.bucket_type"
    );
    size!(RcAchievementList, "rc_client_achievement_list_t");
    size!(RcLeaderboard, "rc_client_leaderboard_t");
    offset!(
        RcLeaderboard,
        lower_is_better,
        "rc_client_leaderboard_t.lower_is_better"
    );
    size!(RcLeaderboardTracker, "rc_client_leaderboard_tracker_t");
    offset!(
        RcLeaderboardTracker,
        id,
        "rc_client_leaderboard_tracker_t.id"
    );
    size!(RcLeaderboardBucket, "rc_client_leaderboard_bucket_t");
    size!(RcLeaderboardList, "rc_client_leaderboard_list_t");
    size!(
        RcScoreboardEntry,
        "rc_client_leaderboard_scoreboard_entry_t"
    );
    size!(RcScoreboard, "rc_client_leaderboard_scoreboard_t");
    offset!(
        RcScoreboard,
        top_entries,
        "rc_client_leaderboard_scoreboard_t.top_entries"
    );
    offset!(
        RcScoreboard,
        num_top_entries,
        "rc_client_leaderboard_scoreboard_t.num_top_entries"
    );
    size!(RcServerError, "rc_client_server_error_t");
    offset!(
        RcServerError,
        related_id,
        "rc_client_server_error_t.related_id"
    );
    size!(RcClientEvent, "rc_client_event_t");
    offset!(RcClientEvent, subset, "rc_client_event_t.subset");
}

#[test]
fn api_struct_layouts_match_c() {
    size!(RcBufferChunk, "rc_buffer_chunk_t");
    size!(RcBuffer, "rc_buffer_t");
    size!(RcApiRequest, "rc_api_request_t");
    offset!(RcApiRequest, buffer, "rc_api_request_t.buffer");
    size!(RcApiServerResponse, "rc_api_server_response_t");
    offset!(
        RcApiServerResponse,
        http_status_code,
        "rc_api_server_response_t.http_status_code"
    );
    size!(RcApiResponse, "rc_api_response_t");
    offset!(RcApiResponse, buffer, "rc_api_response_t.buffer");
    size!(RcApiLoginRequest, "rc_api_login_request_t");
    size!(RcApiLoginResponse, "rc_api_login_response_t");
    offset!(
        RcApiLoginResponse,
        response,
        "rc_api_login_response_t.response"
    );
    size!(RcApiAwardRequest, "rc_api_award_achievement_request_t");
    offset!(
        RcApiAwardRequest,
        seconds_since_unlock,
        "rc_api_award_achievement_request_t.seconds_since_unlock"
    );
    size!(RcApiAwardResponse, "rc_api_award_achievement_response_t");
    offset!(
        RcApiAwardResponse,
        response,
        "rc_api_award_achievement_response_t.response"
    );
    size!(RcApiSubmitRequest, "rc_api_submit_lboard_entry_request_t");
    offset!(
        RcApiSubmitRequest,
        seconds_since_completion,
        "rc_api_submit_lboard_entry_request_t.seconds_since_completion"
    );
    size!(RcApiLboardEntry, "rc_api_lboard_entry_t");
    size!(RcApiSubmitResponse, "rc_api_submit_lboard_entry_response_t");
    offset!(
        RcApiSubmitResponse,
        response,
        "rc_api_submit_lboard_entry_response_t.response"
    );
}

#[test]
fn hash_and_libretro_struct_layouts_match_c() {
    size!(RcHashFilereader, "rc_hash_filereader_t");
    size!(RcHashCdreader, "rc_hash_cdreader_t");
    size!(RcHashCallbacks, "rc_hash_callbacks_t");
    offset!(RcHashCallbacks, cdreader, "rc_hash_callbacks_t.cdreader");
    size!(RcHashIterator, "rc_hash_iterator_t");
    offset!(RcHashIterator, callbacks, "rc_hash_iterator_t.callbacks");
    size!(RcLibretroMemoryRegions, "rc_libretro_memory_regions_t");
    offset!(
        RcLibretroMemoryRegions,
        count,
        "rc_libretro_memory_regions_t.count"
    );
    size!(RcLibretroCoreMemoryInfo, "rc_libretro_core_memory_info_t");
    size!(RcDisallowedSetting, "rc_disallowed_setting_t");
    size!(RetroMemoryMap, "struct retro_memory_map");
    // libretro-host owns the descriptor type; rc_libretro reads it as C lays it out.
    size!(
        libretro_host::RetroMemoryDescriptor,
        "struct retro_memory_descriptor"
    );
    offset!(
        libretro_host::RetroMemoryDescriptor,
        addrspace,
        "struct retro_memory_descriptor.addrspace"
    );
}
