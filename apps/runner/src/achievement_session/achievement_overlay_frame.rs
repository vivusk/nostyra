//! The session's window side: the composed overlay for each present (status line +
//! achievement layers, chime when an unlock popup appears), the badge cache the menu
//! page draws from, and that page's content. Headless runs never call these, so they
//! never fetch badges, parse fonts or open the chime stream.

use std::time::Instant;

use sdl3::AudioSubsystem;
use video_renderer::{OverlayCanvas, ViewportRect};

use super::achievement_event_routing::CATEGORY_CORE;
use super::AchievementSession;
use crate::achievement_overlay::{should_chime, AchievementChime, BadgeCache};
use crate::on_screen_status::{picture_rect, OnScreenStatus};
use crate::pause_menu::AchievementListPage;

/// `RC_CLIENT_ACHIEVEMENT_LIST_GROUPING_PROGRESS`.
const GROUPING_PROGRESS: i32 = 1;

impl AchievementSession {
    /// A window will draw this session: badges are fetched from now on, and the chime
    /// stream opens when `sound` is on and audio exists.
    pub fn attach_window(&mut self, audio: Option<&AudioSubsystem>) {
        self.router.overlay.drawing = true;
        if self.router.overlay.sound() {
            self.chime = audio.and_then(AchievementChime::open);
        }
    }

    /// The overlay to blend at `now` for an `output`-sized window with the picture at
    /// `game`: status text + achievement layers, `None` when nothing shows. A popup
    /// that rings plays the chime unless muted (`sound` off, fast-forward).
    pub fn overlay(
        &mut self,
        (output, game): ((u32, u32), ViewportRect),
        now: Instant,
        status: &mut OnScreenStatus,
        fast_forward: bool,
    ) -> Option<&OverlayCanvas> {
        let overlay = &mut self.router.overlay;
        if should_chime(overlay.tick(now), overlay.sound(), fast_forward) {
            if let Some(chime) = &self.chime {
                chime.play();
            }
        }
        let status = status.current(now);
        self.composer
            .compose(output, picture_rect(game), status, &mut self.router.overlay)
    }

    /// Badges for the pause-menu page.
    pub fn badges_mut(&mut self) -> &mut BadgeCache {
        &mut self.router.overlay.badges
    }

    /// True once after the list changed (set loaded, unlock): an open page rebuilds.
    pub fn take_list_changed(&mut self) -> bool {
        std::mem::take(&mut self.router.list_changed)
    }

    /// The pause-menu page (`None` when the session is inert: no menu item).
    pub fn achievement_list_page(&self) -> Option<AchievementListPage> {
        let client = self.client.as_ref()?;
        let mut game = client.game_info().unwrap_or_default();
        if game.title.is_empty() {
            game.title = if client.is_game_loaded() {
                "Unknown game".into()
            } else {
                "Achievements loading…".into()
            };
        }
        let buckets = client.achievement_list(CATEGORY_CORE, GROUPING_PROGRESS);
        let summary = client.user_game_summary();
        Some(AchievementListPage::build(
            &game,
            &summary,
            client.hardcore_enabled(),
            buckets,
        ))
    }
}
