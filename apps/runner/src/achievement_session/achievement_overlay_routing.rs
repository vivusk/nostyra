//! rc_client runtime events → launcher outputs + overlay model: unlock (IPC event,
//! snapshot, popup with badge + chime), mastery / subset, leaderboard popups (reachable
//! only in hardcore with rcheevos v12.5.0, M15 D3) and scoreboard rank, connection and
//! server messages, trackers and challenge / progress indicators.

use std::time::Instant;

use achievements::{AchievementClient, AchievementEvent, AchievementInfo, LeaderboardInfo};
use ipc_protocol::RunnerEvent;

use super::achievement_event_routing::SessionRouter;
use crate::achievement_overlay::{game_placard, BadgeKey, Popup, PopupKind};

impl SessionRouter {
    pub(super) fn on_event(&mut self, client: &AchievementClient, event: AchievementEvent) {
        let now = Instant::now();
        let indicators = &mut self.overlay.indicators;
        match event {
            AchievementEvent::AchievementTriggered(info) => self.unlocked(client, info),
            AchievementEvent::GameCompleted => {
                let game = client.game_info().unwrap_or_default();
                let user = client
                    .user_info()
                    .map(|u| u.display_name)
                    .unwrap_or_default();
                let played = self.overlay.started.elapsed();
                let popup = game_placard::mastery(&game, client.hardcore_enabled(), &user, played);
                self.overlay.announce(popup);
            }
            AchievementEvent::SubsetCompleted { title, .. } => {
                let header = "Subset completed".to_owned();
                self.overlay.announce(Popup {
                    kind: PopupKind::Mastery,
                    header,
                    title,
                    ..Popup::notice("")
                });
            }
            AchievementEvent::LeaderboardStarted(board) => {
                self.leaderboard("Leaderboard attempt started", board)
            }
            AchievementEvent::LeaderboardFailed(board) => {
                self.leaderboard("Leaderboard attempt failed", board)
            }
            AchievementEvent::LeaderboardSubmitted(board) => {
                self.leaderboard("Leaderboard submitted", board)
            }
            AchievementEvent::Scoreboard(board) => self.overlay.announce(Popup {
                kind: PopupKind::Leaderboard,
                header: "Leaderboard result".into(),
                title: format!("Rank {} of {}", board.new_rank, board.num_entries),
                detail: format!(
                    "Score {} · best {}",
                    board.submitted_score, board.best_score
                ),
                ..Popup::notice("")
            }),
            AchievementEvent::ServerError { api, message, .. } => {
                tracing::warn!(%api, %message, "RetroAchievements server error");
                self.notify(format!("RetroAchievements: {message}"));
            }
            AchievementEvent::Disconnected => self.overlay.announce(Popup {
                detail: "Unlocks will be sent when online".into(),
                ..Popup::notice("RetroAchievements offline")
            }),
            AchievementEvent::Reconnected => self.overlay.announce(Popup {
                detail: "All pending unlocks were sent".into(),
                ..Popup::notice("RetroAchievements reconnected")
            }),
            AchievementEvent::TrackerShow { id, display }
            | AchievementEvent::TrackerUpdate { id, display } => {
                indicators.set_tracker(id, &display)
            }
            AchievementEvent::TrackerHide { id } => indicators.hide_tracker(id),
            AchievementEvent::ChallengeIndicatorShow(info) => {
                indicators.show_challenge(info.id, badge(&info))
            }
            AchievementEvent::ChallengeIndicatorHide(info) => indicators.hide_challenge(info.id),
            AchievementEvent::ProgressIndicatorShow(info)
            | AchievementEvent::ProgressIndicatorUpdate(info) => {
                indicators.show_progress(badge(&info), &info.measured_progress, now)
            }
            AchievementEvent::ProgressIndicatorHide => indicators.hide_progress(),
            AchievementEvent::Reset => {
                tracing::warn!(
                    "rc_client asked for a reset (never expected: no mid-session hardcore)"
                );
            }
        }
    }

    fn unlocked(&mut self, client: &AchievementClient, info: AchievementInfo) {
        self.write_snapshot(client);
        let hardcore = client.hardcore_enabled();
        (self.emit)(&RunnerEvent::AchievementUnlocked {
            id: info.id,
            hardcore,
        });
        tracing::info!(id = info.id, title = %info.title, "achievement unlocked");
        self.list_changed = true;
        self.overlay.indicators.hide_challenge(info.id);
        self.overlay.announce(Popup {
            kind: PopupKind::Unlock,
            icon: Some(badge(&info)),
            header: format!("Achievement unlocked · {} points", info.points),
            title: info.title,
            detail: info.description,
        });
    }

    fn leaderboard(&mut self, header: &str, board: LeaderboardInfo) {
        let detail = match board.tracker_value.is_empty() {
            true => board.description,
            false => format!("Result {}", board.tracker_value),
        };
        self.overlay.announce(Popup {
            kind: PopupKind::Leaderboard,
            header: header.into(),
            title: board.title,
            detail,
            ..Popup::notice("")
        });
    }
}

/// The unlocked badge of `info` (indicators and popups show the earned look).
fn badge(info: &AchievementInfo) -> BadgeKey {
    BadgeKey::Achievement {
        name: info.badge_name.clone(),
        locked: false,
    }
}
