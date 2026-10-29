//! RetroAchievements overlay (M15 phase 05), RetroArch-style, drawn into the CPU
//! overlay canvas the presenter blends after the shader (the core frame, save states
//! and thumbnails never see it): game placard, queued popups with badge + chime,
//! mastery, leaderboard popups, trackers and challenge / progress indicators. Text is
//! bundled Noto Sans (`video_renderer::TtfText`), badges come from the shared cache.
//!
//! [`AchievementOverlay`] is the model the session router feeds from rc_client
//! events; [`OverlayComposer`] paints it together with the status line.

use std::collections::VecDeque;
use std::path::Path;
use std::time::Instant;

mod badge_cache;
mod badge_download;
mod chime;
pub mod game_placard;
mod indicator_view;
mod overlay_composer;
#[cfg(test)]
mod overlay_composer_tests;
#[cfg(test)]
mod overlay_png_render_tests;
mod popup_queue;
mod popup_view;
mod tracker_indicators;

pub use badge_cache::{BadgeCache, BadgeKey};
pub use chime::{should_chime, AchievementChime};
pub use overlay_composer::OverlayComposer;
pub use popup_queue::{Popup, PopupKind, PopupQueue};
pub use tracker_indicators::TrackerIndicators;

/// Translucent box behind popups and indicators.
pub(crate) const BOX_RGBA: [u8; 4] = [16, 18, 26, 225];
pub(crate) const TEXT_RGBA: [u8; 4] = [0xF4, 0xF4, 0xF6, 0xFF];
/// Popup header / list title gold.
pub(crate) const ACCENT_RGBA: [u8; 4] = [255, 204, 102, 255];
/// Log lines kept for headless runs and tests (oldest dropped).
const MAX_LOG: usize = 32;

/// Overlay text size for an output `height`: 18 px at 720p, 27 px at 1080p, 54 px at
/// 4K, never below 12 px.
pub fn ui_px(height: u32) -> u32 {
    (height / 40).clamp(12, 64)
}

/// Overlay state of one session (pure data; drawn by [`OverlayComposer`]).
pub struct AchievementOverlay {
    pub popups: PopupQueue,
    pub indicators: TrackerIndicators,
    pub badges: BadgeCache,
    /// `[achievements] sound` / `leaderboard_popups`.
    sound: bool,
    leaderboard_popups: bool,
    /// A window draws this overlay (badges are fetched only then; headless never).
    pub drawing: bool,
    log: VecDeque<String>,
    /// Session start (mastery playtime).
    pub started: Instant,
}

impl AchievementOverlay {
    pub fn new(data_root: &Path, sound: bool, leaderboard_popups: bool) -> Self {
        Self {
            popups: PopupQueue::default(),
            indicators: TrackerIndicators::default(),
            badges: BadgeCache::new(data_root),
            sound,
            leaderboard_popups,
            drawing: false,
            log: VecDeque::new(),
            started: Instant::now(),
        }
    }

    /// Queues `popup` (leaderboard popups only when enabled) and logs it.
    pub fn announce(&mut self, popup: Popup) {
        if popup.kind == PopupKind::Leaderboard && !self.leaderboard_popups {
            return;
        }
        let line = popup.log_text();
        tracing::info!(text = %line, "achievement popup");
        if self.log.back() != Some(&line) {
            if self.log.len() >= MAX_LOG {
                self.log.pop_front();
            }
            self.log.push_back(line);
        }
        self.popups.push(popup);
    }

    /// Every popup text since the last call (headless log, tests).
    pub fn take_log(&mut self) -> Vec<String> {
        self.log.drain(..).collect()
    }

    /// Advances time at `now` (popup queue, progress linger, landed badges); true
    /// when a popup that rings just appeared ([`should_chime`] adds the mute rules).
    pub fn tick(&mut self, now: Instant) -> bool {
        self.badges.poll();
        self.indicators.tick(now);
        self.popups.tick(now).is_some_and(Popup::chimes)
    }

    pub fn sound(&self) -> bool {
        self.sound
    }

    /// Change counters of the drawn layers (composer redraw key).
    pub fn revisions(&self) -> (u64, u64, u64) {
        (
            self.popups.revision(),
            self.indicators.revision(),
            self.badges.revision(),
        )
    }
}
