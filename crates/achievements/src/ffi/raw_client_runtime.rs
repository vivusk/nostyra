//! Per-frame and query calls of [`RawClient`]: do_frame / idle / reset / can_pause,
//! progress blobs, user / game info, achievement and leaderboard lists (copied into
//! owned values).

use std::ffi::c_int;

use super::client_ffi::*;
use super::client_types::RcUserGameSummary;
use super::event_conversion::{achievement, leaderboard, slice_or_empty};
use super::raw_client::RawClient;
use crate::achievement_info::{
    AchievementBucket, GameInfo, GameSummary, LeaderboardBucket, UserInfo,
};
use crate::ffi::c_text::owned_text;

impl RawClient {
    pub fn do_frame(&self) {
        // SAFETY: live client on its thread.
        unsafe { rc_client_do_frame(self.raw()) }
    }

    pub fn idle(&self) {
        // SAFETY: live client on its thread.
        unsafe { rc_client_idle(self.raw()) }
    }

    pub fn reset(&self) {
        // SAFETY: live client on its thread.
        unsafe { rc_client_reset(self.raw()) }
    }

    /// (pause allowed, frames until it is) — hardcore pause rate limit.
    pub fn can_pause(&self) -> (bool, u32) {
        let mut frames = 0u32;
        // SAFETY: live client; `frames` is a valid out-param.
        let allowed = unsafe { rc_client_can_pause(self.raw(), &mut frames) } != 0;
        (allowed, frames)
    }

    /// Serialized runtime progress; `Err(rc)` while no game is loaded (−29).
    pub fn serialize_progress(&self) -> Result<Vec<u8>, i32> {
        // SAFETY: live client.
        let size = unsafe { rc_client_progress_size(self.raw()) };
        if size == 0 {
            return Err(crate::achievement_events::RC_NO_GAME_LOADED);
        }
        let mut blob = vec![0u8; size];
        // SAFETY: buffer of `size` bytes as reported by rc_client_progress_size.
        let rc = unsafe { rc_client_serialize_progress_sized(self.raw(), blob.as_mut_ptr(), size) };
        if rc == 0 {
            Ok(blob)
        } else {
            Err(rc)
        }
    }

    /// Restores progress; `None` resets every achievement to fresh (state without a blob).
    pub fn deserialize_progress(&self, blob: Option<&[u8]>) -> Result<(), i32> {
        let (ptr, len) = blob.map_or((std::ptr::null(), 0), |b| (b.as_ptr(), b.len()));
        // SAFETY: live client; NULL/0 = reset per rc_client.h, else `len` valid bytes.
        let rc = unsafe { rc_client_deserialize_progress_sized(self.raw(), ptr, len) };
        if rc == 0 {
            Ok(())
        } else {
            Err(rc)
        }
    }

    /// Achievements grouped per `grouping` (0 lock state, 1 progress) for `category`
    /// (1 core, 2 unofficial, 3 both).
    pub fn achievement_list(&self, category: i32, grouping: i32) -> Vec<AchievementBucket> {
        // SAFETY: live client; the list (or NULL) is read here and destroyed once.
        unsafe {
            let list =
                rc_client_create_achievement_list(self.raw(), category as c_int, grouping as c_int);
            let Some(list_ref) = list.as_ref() else {
                return Vec::new();
            };
            let buckets = slice_or_empty(list_ref.buckets, list_ref.num_buckets)
                .iter()
                .map(|b| AchievementBucket {
                    label: owned_text(b.label),
                    subset_id: b.subset_id,
                    bucket_type: b.bucket_type,
                    achievements: slice_or_empty(b.achievements, b.num_achievements)
                        .iter()
                        .filter(|a| !a.is_null())
                        .map(|&a| achievement(a))
                        .collect(),
                })
                .collect();
            rc_client_destroy_achievement_list(list);
            buckets
        }
    }

    /// Leaderboards grouped per `grouping` (0 none, 1 tracking).
    pub fn leaderboard_list(&self, grouping: i32) -> Vec<LeaderboardBucket> {
        // SAFETY: live client; the list (or NULL) is read here and destroyed once.
        unsafe {
            let list = rc_client_create_leaderboard_list(self.raw(), grouping as c_int);
            let Some(list_ref) = list.as_ref() else {
                return Vec::new();
            };
            let buckets = slice_or_empty(list_ref.buckets, list_ref.num_buckets)
                .iter()
                .map(|b| LeaderboardBucket {
                    label: owned_text(b.label),
                    subset_id: b.subset_id,
                    bucket_type: b.bucket_type,
                    leaderboards: slice_or_empty(b.leaderboards, b.num_leaderboards)
                        .iter()
                        .filter(|l| !l.is_null())
                        .map(|&l| leaderboard(l))
                        .collect(),
                })
                .collect();
            rc_client_destroy_leaderboard_list(list);
            buckets
        }
    }

    pub fn user_info(&self) -> Option<UserInfo> {
        // SAFETY: live client; the returned struct (or NULL) is valid until the next call.
        unsafe {
            let user = rc_client_get_user_info(self.raw()).as_ref()?;
            Some(UserInfo {
                display_name: owned_text(user.display_name),
                username: owned_text(user.username),
                score: user.score,
                score_softcore: user.score_softcore,
            })
        }
    }

    pub fn game_info(&self) -> Option<GameInfo> {
        // SAFETY: live client; the returned struct (or NULL) is valid until the next call.
        unsafe {
            let game = rc_client_get_game_info(self.raw()).as_ref()?;
            Some(GameInfo {
                id: game.id,
                console_id: game.console_id,
                title: owned_text(game.title),
                hash: owned_text(game.hash),
                badge_name: owned_text(game.badge_name),
            })
        }
    }

    pub fn user_game_summary(&self) -> GameSummary {
        let mut s = RcUserGameSummary::default();
        // SAFETY: live client; `s` is a valid out-param.
        unsafe { rc_client_get_user_game_summary(self.raw(), &mut s) };
        GameSummary {
            num_core_achievements: s.num_core_achievements,
            num_unofficial_achievements: s.num_unofficial_achievements,
            num_unlocked_achievements: s.num_unlocked_achievements,
            num_unsupported_achievements: s.num_unsupported_achievements,
            points_core: s.points_core,
            points_unlocked: s.points_unlocked,
        }
    }
}
