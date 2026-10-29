//! Achievement popups: one visible at a time, first in first out, each for a fixed
//! time (placard 5 s, mastery 6 s, others 4 s). Static boxes (no slide animation), so
//! the overlay canvas redraws only when the visible popup changes.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::badge_cache::BadgeKey;

/// What a popup announces (decides its time on screen and the chime).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupKind {
    /// Game start: title, counts, mode (or why achievements are inactive).
    Placard,
    Unlock,
    /// Game mastered (hardcore) / completed (softcore), subset completed.
    Mastery,
    /// Leaderboard started / failed / submitted / rank (hidden when the
    /// `leaderboard_popups` setting is off).
    Leaderboard,
    /// Connection, server and login messages, hardcore drops.
    Notice,
}

/// One popup: optional icon, a small header line, a bold title and a detail text.
#[derive(Clone, Debug, PartialEq)]
pub struct Popup {
    pub kind: PopupKind,
    pub icon: Option<BadgeKey>,
    pub header: String,
    pub title: String,
    pub detail: String,
}

impl Popup {
    /// A plain message (`title` only, "RetroAchievements" header).
    pub fn notice(title: impl Into<String>) -> Self {
        Self {
            kind: PopupKind::Notice,
            icon: None,
            header: "RetroAchievements".into(),
            title: title.into(),
            detail: String::new(),
        }
    }

    pub fn duration(&self) -> Duration {
        match self.kind {
            PopupKind::Placard => Duration::from_secs(5),
            PopupKind::Mastery => Duration::from_secs(6),
            _ => Duration::from_secs(4),
        }
    }

    /// Unlocks and masteries ring the chime (when sound is on).
    pub fn chimes(&self) -> bool {
        matches!(self.kind, PopupKind::Unlock | PopupKind::Mastery)
    }

    /// One log line (headless runs, tests): a notice is its title, others
    /// "header: title".
    pub fn log_text(&self) -> String {
        match self.kind {
            PopupKind::Notice => self.title.clone(),
            _ => format!("{}: {}", self.header, self.title),
        }
    }
}

/// Waiting popups beyond this are dropped oldest first (a burst never piles up
/// minutes of popups; headless runs never show them).
const MAX_WAITING: usize = 32;

#[derive(Default)]
pub struct PopupQueue {
    waiting: VecDeque<Popup>,
    shown: Option<(Popup, Instant)>,
    /// Bumped whenever the visible popup changes.
    revision: u64,
}

impl PopupQueue {
    pub fn push(&mut self, popup: Popup) {
        if self.waiting.len() >= MAX_WAITING {
            self.waiting.pop_front();
        }
        self.waiting.push_back(popup);
    }

    /// Expires the visible popup and shows the next one at `now`; returns the popup
    /// that just appeared (the caller rings the chime when it [`Popup::chimes`]).
    pub fn tick(&mut self, now: Instant) -> Option<&Popup> {
        if self.shown.as_ref().is_some_and(|(_, until)| now >= *until) {
            self.shown = None;
            self.revision += 1;
        }
        if self.shown.is_some() {
            return None;
        }
        let next = self.waiting.pop_front()?;
        let until = now + next.duration();
        self.revision += 1;
        let (shown, _) = self.shown.insert((next, until));
        Some(shown)
    }

    pub fn current(&self) -> Option<&Popup> {
        self.shown.as_ref().map(|(popup, _)| popup)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn popup(kind: PopupKind, title: &str) -> Popup {
        Popup {
            kind,
            title: title.into(),
            ..Popup::notice("")
        }
    }

    #[test]
    fn popups_show_in_order_for_their_duration() {
        let mut queue = PopupQueue::default();
        let t0 = Instant::now();
        assert!(queue.tick(t0).is_none() && queue.current().is_none());
        queue.push(popup(PopupKind::Placard, "game"));
        queue.push(popup(PopupKind::Unlock, "first"));
        queue.push(popup(PopupKind::Mastery, "mastered"));
        assert!(!queue.tick(t0).unwrap().chimes(), "placard does not chime");
        assert_eq!(queue.current().unwrap().title, "game");
        let rev = queue.revision();
        assert!(queue.tick(t0 + Duration::from_millis(4900)).is_none());
        assert_eq!(queue.revision(), rev, "no change while it stays");
        let t1 = t0 + Duration::from_secs(5);
        let first = queue.tick(t1).unwrap();
        assert!(first.title == "first" && first.chimes(), "unlock chimes");
        assert!(queue.tick(t1 + Duration::from_millis(3999)).is_none());
        let t2 = t1 + Duration::from_secs(4);
        assert_eq!(queue.tick(t2).unwrap().title, "mastered");
        assert!(queue.tick(t2 + Duration::from_millis(5999)).is_none());
        assert!(queue.current().is_some(), "mastery stays 6 s");
        queue.tick(t2 + Duration::from_secs(6));
        assert!(queue.current().is_none());
    }

    #[test]
    fn a_burst_keeps_the_newest_popups() {
        let mut queue = PopupQueue::default();
        for i in 0..40 {
            queue.push(popup(PopupKind::Notice, &i.to_string()));
        }
        queue.tick(Instant::now());
        assert_eq!(queue.current().unwrap().title, "8", "oldest 8 dropped");
        assert_eq!(popup(PopupKind::Notice, "x").log_text(), "x");
        let unlock = Popup {
            header: "Achievement unlocked".into(),
            ..popup(PopupKind::Unlock, "Coins")
        };
        assert_eq!(unlock.log_text(), "Achievement unlocked: Coins");
    }
}
