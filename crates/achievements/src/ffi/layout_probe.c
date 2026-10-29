/* Size / offset probe for the hand-written rcheevos FFI (crates/achievements/src/ffi).
 * Compiled by build.rs with the same defines as the library, so `ffi_layout_tests`
 * compares the Rust #[repr(C)] mirrors with what this C compiler really lays out. */

#include <stddef.h>
#include <string.h>

#include "rc_api_runtime.h"
#include "rc_api_user.h"
#include "rc_client.h"
#include "rc_hash.h"
#include "rc_libretro.h"

#define SIZE(t) if (strcmp(key, #t) == 0) return sizeof(t);
#define OFFSET(t, f) if (strcmp(key, #t "." #f) == 0) return offsetof(t, f);

/* Returns sizeof / offsetof for "type" or "type.field"; (size_t)-1 when unknown. */
size_t nostyra_rc_layout(const char* key)
{
  SIZE(rc_client_user_t) OFFSET(rc_client_user_t, avatar_last_updated)
  SIZE(rc_client_user_game_summary_t) OFFSET(rc_client_user_game_summary_t, completed_time)
  SIZE(rc_client_game_t) OFFSET(rc_client_game_t, badge_url)
  SIZE(rc_client_subset_t) OFFSET(rc_client_subset_t, badge_url)
  SIZE(rc_client_achievement_t)
  OFFSET(rc_client_achievement_t, measured_percent) OFFSET(rc_client_achievement_t, unlock_time)
  OFFSET(rc_client_achievement_t, unlocked) OFFSET(rc_client_achievement_t, type)
  OFFSET(rc_client_achievement_t, badge_locked_url)
  SIZE(rc_client_achievement_bucket_t) OFFSET(rc_client_achievement_bucket_t, bucket_type)
  SIZE(rc_client_achievement_list_t)
  SIZE(rc_client_leaderboard_t) OFFSET(rc_client_leaderboard_t, lower_is_better)
  SIZE(rc_client_leaderboard_tracker_t) OFFSET(rc_client_leaderboard_tracker_t, id)
  SIZE(rc_client_leaderboard_bucket_t) SIZE(rc_client_leaderboard_list_t)
  SIZE(rc_client_leaderboard_scoreboard_entry_t)
  SIZE(rc_client_leaderboard_scoreboard_t) OFFSET(rc_client_leaderboard_scoreboard_t, top_entries)
  OFFSET(rc_client_leaderboard_scoreboard_t, num_top_entries)
  SIZE(rc_client_server_error_t) OFFSET(rc_client_server_error_t, related_id)
  SIZE(rc_client_event_t) OFFSET(rc_client_event_t, subset)
  SIZE(rc_buffer_chunk_t) SIZE(rc_buffer_t)
  SIZE(rc_api_request_t) OFFSET(rc_api_request_t, buffer)
  SIZE(rc_api_server_response_t) OFFSET(rc_api_server_response_t, http_status_code)
  SIZE(rc_api_response_t) OFFSET(rc_api_response_t, buffer)
  SIZE(rc_api_login_request_t)
  SIZE(rc_api_login_response_t) OFFSET(rc_api_login_response_t, response)
  SIZE(rc_api_award_achievement_request_t)
  OFFSET(rc_api_award_achievement_request_t, seconds_since_unlock)
  SIZE(rc_api_award_achievement_response_t) OFFSET(rc_api_award_achievement_response_t, response)
  SIZE(rc_api_submit_lboard_entry_request_t)
  OFFSET(rc_api_submit_lboard_entry_request_t, seconds_since_completion)
  SIZE(rc_api_lboard_entry_t)
  SIZE(rc_api_submit_lboard_entry_response_t) OFFSET(rc_api_submit_lboard_entry_response_t, response)
  SIZE(rc_hash_filereader_t) SIZE(rc_hash_cdreader_t)
  SIZE(rc_hash_callbacks_t) OFFSET(rc_hash_callbacks_t, cdreader)
  SIZE(rc_hash_iterator_t) OFFSET(rc_hash_iterator_t, callbacks)
  SIZE(rc_libretro_memory_regions_t) OFFSET(rc_libretro_memory_regions_t, count)
  SIZE(rc_libretro_core_memory_info_t)
  SIZE(rc_disallowed_setting_t)
  SIZE(struct retro_memory_map)
  SIZE(struct retro_memory_descriptor) OFFSET(struct retro_memory_descriptor, addrspace)
  return (size_t)-1;
}
