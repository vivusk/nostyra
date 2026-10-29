//! Core messages (`SET_MESSAGE(_EXT)`, e.g. "No PlayStation BIOS file found") shown
//! as on-screen notices; the same text at most once every 5 s so a core repeating
//! itself cannot flood the picture. Every message is also in the log (host side).

use std::time::{Duration, Instant};

use super::EmulationApp;

const REPEAT_AFTER: Duration = Duration::from_secs(5);

/// Remembers the last notice shown and when.
#[derive(Default)]
pub(crate) struct CoreNotices {
    last: Option<(String, Instant)>,
}

impl CoreNotices {
    /// True when `text` may be shown now (new text, or 5 s since it was last shown).
    fn admit(&mut self, text: &str, now: Instant) -> bool {
        let repeat = self
            .last
            .as_ref()
            .is_some_and(|(last, at)| last == text && now.duration_since(*at) < REPEAT_AFTER);
        if !repeat {
            self.last = Some((text.to_owned(), now));
        }
        !repeat
    }
}

impl EmulationApp<'_> {
    /// Moves pending core messages to the on-screen status line.
    pub(super) fn show_core_messages(&mut self) {
        let now = Instant::now();
        for text in self.session.take_core_messages() {
            if self.core_notices.admit(&text, now) {
                self.status.show(text, now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_text_waits_five_seconds_other_text_does_not() {
        let mut notices = CoreNotices::default();
        let t0 = Instant::now();
        assert!(notices.admit("No BIOS", t0));
        assert!(!notices.admit("No BIOS", t0 + Duration::from_secs(2)));
        assert!(notices.admit("Disc 2 inserted", t0 + Duration::from_secs(2)));
        assert!(notices.admit("No BIOS", t0 + Duration::from_secs(3)));
        assert!(!notices.admit("No BIOS", t0 + Duration::from_secs(7)));
        assert!(notices.admit("No BIOS", t0 + Duration::from_secs(9)));
    }
}
