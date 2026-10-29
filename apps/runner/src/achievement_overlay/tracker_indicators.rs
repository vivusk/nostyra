//! Persistent achievement indicators (bottom-right of the picture): leaderboard
//! trackers (formatted value text copied on every update), challenge indicators
//! (badge per achievement id while its challenge is live) and one progress indicator
//! (badge + measured text) that hides 2 s after its last update. Each change bumps
//! the revision so the canvas redraws only then.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::badge_cache::BadgeKey;

/// The progress indicator hides this long after its last show / update.
pub const PROGRESS_LINGER: Duration = Duration::from_secs(2);

/// Badge + progress text such as "5/10".
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressIndicator {
    pub badge: BadgeKey,
    pub text: String,
    updated: Instant,
}

#[derive(Default)]
pub struct TrackerIndicators {
    trackers: BTreeMap<u32, String>,
    challenges: BTreeMap<u32, BadgeKey>,
    progress: Option<ProgressIndicator>,
    revision: u64,
}

impl TrackerIndicators {
    /// Tracker `id` shown or updated with `display` (no redraw when unchanged).
    pub fn set_tracker(&mut self, id: u32, display: &str) {
        if self.trackers.get(&id).map(String::as_str) != Some(display) {
            self.trackers.insert(id, display.to_owned());
            self.revision += 1;
        }
    }

    pub fn hide_tracker(&mut self, id: u32) {
        if self.trackers.remove(&id).is_some() {
            self.revision += 1;
        }
    }

    pub fn show_challenge(&mut self, id: u32, badge: BadgeKey) {
        if self.challenges.insert(id, badge.clone()) != Some(badge) {
            self.revision += 1;
        }
    }

    pub fn hide_challenge(&mut self, id: u32) {
        if self.challenges.remove(&id).is_some() {
            self.revision += 1;
        }
    }

    /// Progress shown / updated at `now` (restarts the linger time).
    pub fn show_progress(&mut self, badge: BadgeKey, text: &str, now: Instant) {
        let changed = self
            .progress
            .as_ref()
            .is_none_or(|p| p.badge != badge || p.text != text);
        self.progress = Some(ProgressIndicator {
            badge,
            text: text.to_owned(),
            updated: now,
        });
        if changed {
            self.revision += 1;
        }
    }

    pub fn hide_progress(&mut self) {
        if self.progress.take().is_some() {
            self.revision += 1;
        }
    }

    /// Hides a progress indicator not updated for [`PROGRESS_LINGER`].
    pub fn tick(&mut self, now: Instant) {
        if self
            .progress
            .as_ref()
            .is_some_and(|p| now >= p.updated + PROGRESS_LINGER)
        {
            self.hide_progress();
        }
    }

    pub fn trackers(&self) -> impl Iterator<Item = &str> {
        self.trackers.values().map(String::as_str)
    }

    pub fn challenges(&self) -> impl Iterator<Item = &BadgeKey> {
        self.challenges.values()
    }

    pub fn progress(&self) -> Option<&ProgressIndicator> {
        self.progress.as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.trackers.is_empty() && self.challenges.is_empty() && self.progress.is_none()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn badge(name: &str) -> BadgeKey {
        BadgeKey::Achievement {
            name: name.into(),
            locked: false,
        }
    }

    #[test]
    fn trackers_and_challenges_follow_show_update_hide() {
        let mut ind = TrackerIndicators::default();
        assert!(ind.is_empty());
        ind.set_tracker(7, "0:12.50");
        ind.set_tracker(3, "120");
        let rev = ind.revision();
        ind.set_tracker(7, "0:12.50");
        assert_eq!(ind.revision(), rev, "same text: no redraw");
        ind.set_tracker(7, "0:13.00");
        assert_eq!(ind.trackers().collect::<Vec<_>>(), ["120", "0:13.00"]);
        ind.hide_tracker(3);
        ind.hide_tracker(3);
        ind.show_challenge(101, badge("00101"));
        ind.show_challenge(101, badge("00101"));
        assert_eq!(ind.challenges().count(), 1);
        ind.hide_challenge(101);
        ind.hide_tracker(7);
        assert!(ind.is_empty());
    }

    #[test]
    fn progress_hides_two_seconds_after_the_last_update() {
        let mut ind = TrackerIndicators::default();
        let t0 = Instant::now();
        ind.show_progress(badge("00102"), "3/10", t0);
        ind.tick(t0 + Duration::from_millis(1500));
        ind.show_progress(badge("00102"), "4/10", t0 + Duration::from_millis(1500));
        ind.tick(t0 + Duration::from_millis(3000));
        assert_eq!(
            ind.progress().unwrap().text,
            "4/10",
            "update restarted the timer"
        );
        ind.tick(t0 + Duration::from_millis(3500));
        assert!(ind.progress().is_none());
        let rev = ind.revision();
        ind.hide_progress();
        assert_eq!(ind.revision(), rev);
    }
}
