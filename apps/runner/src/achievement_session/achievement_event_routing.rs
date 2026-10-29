//! Routes what rc_client reports, in this order per pump: request observations
//! (offline journal first, M15 D-journal), `begin_*` completions (login, game load,
//! disc change), then runtime events (`achievement_overlay_routing`). Outputs: IPC
//! events + snapshot for the launcher, and the overlay model (placard, popups,
//! indicators, badge downloads) the window draws.

use std::path::Path;

use achievements::{
    AchievementClient, ClientCompletion, ClientOp, GameInfo, RC_HARDCORE_DISABLED,
    RC_LOGIN_REQUIRED, RC_NO_GAME_LOADED,
};
use ipc_protocol::RunnerEvent;

use super::achievement_journal_bridge::JournalBridge;
use super::achievement_snapshot_writer::SnapshotTarget;
use super::patched_rom_fallback::PatchedRomFallback;
use super::EventSink;
use crate::achievement_overlay::{game_placard, AchievementOverlay, BadgeKey, Popup};

/// `RC_CLIENT_ACHIEVEMENT_CATEGORY_CORE` (lists and badges: the official set only).
pub(super) const CATEGORY_CORE: i32 = 1;
/// Message when the server rejected the stored token.
pub(super) const LOGIN_AGAIN: &str = "RetroAchievements: please log in again in Settings";

pub(super) struct SessionRouter {
    journal: JournalBridge,
    snapshot: SnapshotTarget,
    pub(super) emit: EventSink,
    /// Placard, popups, indicators and badges (drawn by the window).
    pub(super) overlay: AchievementOverlay,
    /// The achievement list changed (set loaded, unlock): an open menu page rebuilds.
    pub(super) list_changed: bool,
    /// Last plain message (an identical one right after is dropped).
    last_notice: Option<String>,
    /// Progress from a state loaded while the game set was still loading (last
    /// wins; `Some(None)` = reset), applied when the load completes.
    pub(super) pending_progress: Option<Option<Vec<u8>>>,
    /// The token was rejected: the session stops (launcher logs the user out).
    pub(super) login_expired: bool,
    /// A login failure was shown; the load's follow-up "Login required" is not.
    login_failed: bool,
    /// Original-ROM retry of an unknown soft-patched game (`patched_rom_fallback`).
    pub(super) patched_fallback: Option<PatchedRomFallback>,
}

impl SessionRouter {
    pub(super) fn new(
        data_root: &Path,
        (content_id, user): (&str, &str),
        overlay: AchievementOverlay,
        emit: EventSink,
    ) -> Self {
        Self {
            journal: JournalBridge::new(data_root, user),
            snapshot: SnapshotTarget::new(data_root, content_id),
            emit,
            overlay,
            list_changed: false,
            last_notice: None,
            pending_progress: None,
            login_expired: false,
            login_failed: false,
            patched_fallback: None,
        }
    }

    pub(super) fn route(&mut self, client: &AchievementClient) {
        for observation in client.drain_request_observations() {
            self.journal.observe(observation);
        }
        for completion in client.drain_completions() {
            self.on_completion(client, completion);
        }
        for event in client.drain_events() {
            self.on_event(client, event);
        }
    }

    fn on_completion(&mut self, client: &AchievementClient, done: ClientCompletion) {
        if done.is_login_rejected() {
            return self.expire();
        }
        match (done.op, done.result) {
            (op, achievements::RC_OK) => {
                if op != ClientOp::LoginWithToken {
                    // A disc swap may switch to another set; only a game load shows the placard.
                    self.game_loaded(client, op == ClientOp::LoadGame);
                }
            }
            (ClientOp::LoadGame, RC_NO_GAME_LOADED) => {
                let hash = client.game_info().map(|g| g.hash).unwrap_or_default();
                if self.retry_with_original_rom(client, &hash) {
                    return; // pending progress is kept for the retry
                }
                let hash = self.unknown_placard_hash(hash);
                tracing::info!(%hash, "game not known to RetroAchievements");
                self.overlay.announce(game_placard::unknown(&hash));
                self.pending_progress = None;
            }
            (ClientOp::ChangeMedia, RC_HARDCORE_DISABLED) => {
                self.notify("Hardcore disabled: unknown disc".into());
            }
            (op, result) => {
                tracing::warn!(?op, result, message = %done.message, "achievement request failed");
                let after_login_failure = self.login_failed && result == RC_LOGIN_REQUIRED;
                self.login_failed |= op == ClientOp::LoginWithToken;
                if op != ClientOp::ChangeMedia {
                    if !after_login_failure {
                        self.notify(format!("RetroAchievements: {}", done.message));
                    }
                    self.pending_progress = None;
                }
            }
        }
    }

    /// Set loaded: pending state progress first (before any frame), then the snapshot,
    /// the launcher event, badge downloads and (game load only) the placard.
    fn game_loaded(&mut self, client: &AchievementClient, placard: bool) {
        if let Some(blob) = self.pending_progress.take() {
            if let Err(rc) = client.deserialize_progress(blob.as_deref()) {
                tracing::warn!(rc, "pending achievement progress not restored");
            }
        }
        let Some(summary) = self.snapshot.write(client) else {
            return;
        };
        let game = client.game_info().unwrap_or_default();
        let hardcore = client.hardcore_enabled();
        (self.emit)(&RunnerEvent::AchievementsLoaded {
            ra_game_id: game.id,
            unlocked: summary.num_unlocked_achievements,
            total: summary.num_core_achievements,
            hardcore,
        });
        self.list_changed = true;
        self.request_badges(client, &game);
        if placard {
            self.announce_patched_fallback();
            let popup = game_placard::loaded(&game, &summary, hardcore);
            self.overlay.announce(popup);
        }
    }

    /// Every badge (unlocked + locked variants) and the game icon, so popups and the
    /// list find them on disk. Window sessions only (headless never draws).
    fn request_badges(&mut self, client: &AchievementClient, game: &GameInfo) {
        if !self.overlay.drawing {
            return;
        }
        let badges = &mut self.overlay.badges;
        if let Some(icon) = game_placard::game_icon(game) {
            badges.request(icon, client);
        }
        let buckets = client.achievement_list(CATEGORY_CORE, 0);
        for achievement in buckets.into_iter().flat_map(|b| b.achievements) {
            for locked in [false, true] {
                let name = achievement.badge_name.clone();
                badges.request(BadgeKey::Achievement { name, locked }, client);
            }
        }
    }

    /// The snapshot after an unlock (launcher tab).
    pub(super) fn write_snapshot(&mut self, client: &AchievementClient) {
        self.snapshot.write(client);
    }

    /// A plain message popup, unless it repeats the last one (offline at start fails
    /// login and load with the same text).
    pub(super) fn notify(&mut self, text: String) {
        if self.last_notice.as_deref() != Some(text.as_str()) {
            self.overlay.announce(Popup::notice(text.clone()));
            self.last_notice = Some(text);
        }
    }

    fn expire(&mut self) {
        if !self.login_expired {
            self.login_expired = true;
            tracing::warn!("RetroAchievements rejected the stored token");
            self.notify(LOGIN_AGAIN.into());
            (self.emit)(&RunnerEvent::AchievementsLoginExpired);
        }
    }
}
