//! What a running session reports to the launcher ([`SessionUpdate`]) and how it
//! ended ([`SessionOutcome`]); runner achievement events map to one
//! [`SessionUpdate::Achievements`] variant so each launcher `match` grows by one arm.

use ipc_protocol::RunnerEvent;
use serde::Serialize;

use crate::runner_exit_classification::SessionErrorKind;

/// Progress of a running session, in order: `Started`?, `LaunchTimedOut`?,
/// `Achievements`*, `Finished`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SessionUpdate {
    Started {
        core: String,
        rom: String,
    },
    /// No `Started` within the launch timeout; the process may still start or exit.
    LaunchTimedOut,
    /// RetroAchievements news from the runner (snapshot file already rewritten).
    Achievements(AchievementUpdate),
    Finished(SessionOutcome),
}

/// RetroAchievements events of a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AchievementUpdate {
    /// The game's set loaded (counts of core achievements).
    Loaded {
        ra_game_id: u32,
        unlocked: u32,
        total: u32,
        hardcore: bool,
    },
    Unlocked {
        id: u32,
        hardcore: bool,
    },
    /// The server rejected the stored token: the launcher logs the user out.
    LoginExpired,
}

impl AchievementUpdate {
    /// The update for a runner event; `None` for non-achievement events.
    pub fn from_event(event: RunnerEvent) -> Option<Self> {
        Some(match event {
            RunnerEvent::AchievementsLoaded {
                ra_game_id,
                unlocked,
                total,
                hardcore,
            } => Self::Loaded {
                ra_game_id,
                unlocked,
                total,
                hardcore,
            },
            RunnerEvent::AchievementUnlocked { id, hardcore } => Self::Unlocked { id, hardcore },
            RunnerEvent::AchievementsLoginExpired => Self::LoginExpired,
            _ => return None,
        })
    }
}

/// How a session ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOutcome {
    /// `None` = killed by a signal.
    pub exit_code: Option<i32>,
    /// Unix seconds when `Started` arrived; `None` = never started → no play session.
    pub started_at: Option<i64>,
    /// Launcher-measured `Started` → exit; used only when no pending file was written.
    pub measured_seconds: u64,
    pub error: Option<SessionError>,
}

/// A failed session, for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionError {
    pub code: Option<i32>,
    pub kind: SessionErrorKind,
    /// Runner `Error` message, else its last stderr lines.
    pub detail: String,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal
    use super::*;

    #[test]
    fn achievement_events_map_and_others_do_not() {
        let loaded = RunnerEvent::AchievementsLoaded {
            ra_game_id: 7,
            unlocked: 1,
            total: 3,
            hardcore: false,
        };
        assert_eq!(
            AchievementUpdate::from_event(loaded),
            Some(AchievementUpdate::Loaded {
                ra_game_id: 7,
                unlocked: 1,
                total: 3,
                hardcore: false
            })
        );
        assert_eq!(
            AchievementUpdate::from_event(RunnerEvent::AchievementUnlocked {
                id: 5,
                hardcore: true
            }),
            Some(AchievementUpdate::Unlocked {
                id: 5,
                hardcore: true
            })
        );
        assert_eq!(
            AchievementUpdate::from_event(RunnerEvent::AchievementsLoginExpired),
            Some(AchievementUpdate::LoginExpired)
        );
        assert_eq!(
            AchievementUpdate::from_event(RunnerEvent::Stopped { play_seconds: 1 }),
            None
        );
    }

    #[test]
    fn serializes_with_both_tags() {
        let update = SessionUpdate::Achievements(AchievementUpdate::LoginExpired);
        let json = serde_json::to_string(&update).unwrap();
        assert_eq!(json, r#"{"type":"achievements","kind":"loginExpired"}"#);
    }
}
