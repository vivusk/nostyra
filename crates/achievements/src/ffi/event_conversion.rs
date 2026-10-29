//! Copies rc_client structs (events, achievements, leaderboards, lists) into the owned
//! Rust types of `achievement_info` / `achievement_events`, inside the callback or
//! call that lent the pointers.

use super::c_text::{fixed_text, owned_text};
use super::client_ffi::event_type;
use super::client_types::*;
use crate::achievement_events::AchievementEvent;
use crate::achievement_info::*;

/// # Safety
/// `a` points to a valid `rc_client_achievement_t` for this call.
pub unsafe fn achievement(a: *const RcAchievement) -> AchievementInfo {
    // SAFETY: caller guarantees validity; string members are NULL or NUL-terminated.
    unsafe {
        let a = &*a;
        AchievementInfo {
            id: a.id,
            title: owned_text(a.title),
            description: owned_text(a.description),
            badge_name: fixed_text(&a.badge_name),
            measured_progress: fixed_text(&a.measured_progress),
            measured_percent: a.measured_percent,
            points: a.points,
            unlock_time: a.unlock_time,
            state: a.state,
            category: a.category,
            bucket: a.bucket,
            unlocked: a.unlocked,
            rarity: a.rarity,
            rarity_hardcore: a.rarity_hardcore,
            kind: a.kind,
        }
    }
}

/// # Safety
/// `l` points to a valid `rc_client_leaderboard_t` for this call.
pub unsafe fn leaderboard(l: *const RcLeaderboard) -> LeaderboardInfo {
    // SAFETY: caller guarantees validity; string members are NULL or NUL-terminated.
    unsafe {
        let l = &*l;
        LeaderboardInfo {
            id: l.id,
            title: owned_text(l.title),
            description: owned_text(l.description),
            tracker_value: owned_text(l.tracker_value),
            state: l.state,
            format: l.format,
            lower_is_better: l.lower_is_better != 0,
        }
    }
}

/// # Safety
/// `s` points to a valid scoreboard whose `top_entries` holds `num_top_entries` rows.
unsafe fn scoreboard(s: *const RcScoreboard) -> Scoreboard {
    // SAFETY: caller guarantees validity of the struct and its entry array.
    unsafe {
        let s = &*s;
        let entries = slice_or_empty(s.top_entries, s.num_top_entries);
        Scoreboard {
            leaderboard_id: s.leaderboard_id,
            submitted_score: fixed_text(&s.submitted_score),
            best_score: fixed_text(&s.best_score),
            new_rank: s.new_rank,
            num_entries: s.num_entries,
            top_entries: entries
                .iter()
                .map(|e| ScoreboardEntry {
                    username: owned_text(e.username),
                    rank: e.rank,
                    score: fixed_text(&e.score),
                })
                .collect(),
        }
    }
}

/// `count` items at `ptr` (NULL / 0 → empty).
///
/// # Safety
/// Non-null `ptr` points to at least `count` valid items for the returned lifetime.
pub unsafe fn slice_or_empty<'a, T>(ptr: *const T, count: u32) -> &'a [T] {
    if ptr.is_null() || count == 0 {
        return &[];
    }
    // SAFETY: caller guarantees `count` valid items.
    unsafe { std::slice::from_raw_parts(ptr, count as usize) }
}

/// Owned copy of one event; `None` for unknown types or a missing payload pointer.
///
/// # Safety
/// `ev` and its members are valid for this call (the event handler's contract).
pub unsafe fn event(ev: *const RcClientEvent) -> Option<AchievementEvent> {
    use event_type as t;
    use AchievementEvent as E;
    // SAFETY: rc_client sets the payload pointer that matches the event type; each is
    // checked for NULL before use.
    unsafe {
        let ev = &*ev;
        let ach = || (!ev.achievement.is_null()).then(|| achievement(ev.achievement));
        let lb = || (!ev.leaderboard.is_null()).then(|| leaderboard(ev.leaderboard));
        let tracker = || {
            (!ev.leaderboard_tracker.is_null()).then(|| {
                let tr = &*ev.leaderboard_tracker;
                (tr.id, fixed_text(&tr.display))
            })
        };
        Some(match ev.kind {
            t::ACHIEVEMENT_TRIGGERED => E::AchievementTriggered(ach()?),
            t::LEADERBOARD_STARTED => E::LeaderboardStarted(lb()?),
            t::LEADERBOARD_FAILED => E::LeaderboardFailed(lb()?),
            t::LEADERBOARD_SUBMITTED => E::LeaderboardSubmitted(lb()?),
            t::CHALLENGE_INDICATOR_SHOW => E::ChallengeIndicatorShow(ach()?),
            t::CHALLENGE_INDICATOR_HIDE => E::ChallengeIndicatorHide(ach()?),
            t::PROGRESS_INDICATOR_SHOW => E::ProgressIndicatorShow(ach()?),
            t::PROGRESS_INDICATOR_HIDE => E::ProgressIndicatorHide,
            t::PROGRESS_INDICATOR_UPDATE => E::ProgressIndicatorUpdate(ach()?),
            t::LEADERBOARD_TRACKER_SHOW => {
                tracker().map(|(id, display)| E::TrackerShow { id, display })?
            }
            t::LEADERBOARD_TRACKER_HIDE => tracker().map(|(id, _)| E::TrackerHide { id })?,
            t::LEADERBOARD_TRACKER_UPDATE => {
                tracker().map(|(id, display)| E::TrackerUpdate { id, display })?
            }
            t::LEADERBOARD_SCOREBOARD if !ev.leaderboard_scoreboard.is_null() => {
                E::Scoreboard(scoreboard(ev.leaderboard_scoreboard))
            }
            t::RESET => E::Reset,
            t::GAME_COMPLETED => E::GameCompleted,
            t::SERVER_ERROR if !ev.server_error.is_null() => {
                let err = &*ev.server_error;
                E::ServerError {
                    api: owned_text(err.api),
                    message: owned_text(err.error_message),
                    result: err.result,
                    related_id: err.related_id,
                }
            }
            t::DISCONNECTED => E::Disconnected,
            t::RECONNECTED => E::Reconnected,
            t::SUBSET_COMPLETED if !ev.subset.is_null() => {
                let subset = &*ev.subset;
                E::SubsetCompleted {
                    id: subset.id,
                    title: owned_text(subset.title),
                }
            }
            _ => return None,
        })
    }
}
