//! Periodic resume point: while a game runs, the `auto` pseudo-slot is also saved
//! every minute of wall-clock time (every 3 minutes on systems with large states:
//! PSP, Dreamcast, GameCube, Wii), so a crash, kill or power loss loses at most that
//! much. Wall-clock, not frames: fast-forward must not save more often. Checked only
//! while frames run (never under the pause menu; a point overdue there is saved on
//! the first redraw after it closes). Same switch and minimum as the exit save
//! (`resume.auto_save`, [`AUTO_SAVE_MIN_FRAMES`] real frames); the exit save still
//! writes the final point.

use std::time::{Duration, Instant};

use crate::auto_save_on_exit::AUTO_SAVE_MIN_FRAMES;

/// Time between two periodic saves on most systems.
pub const INTERVAL: Duration = Duration::from_secs(60);
/// Time between two periodic saves on [`LARGE_STATE_SYSTEMS`].
pub const LARGE_STATE_INTERVAL: Duration = Duration::from_secs(180);
/// Systems whose states are tens of MB or more (a save stalls one frame longer).
const LARGE_STATE_SYSTEMS: [&str; 4] = ["psp", "dreamcast", "gc", "wii"];

/// When the next periodic save is due.
#[derive(Debug)]
pub struct PeriodicResumeSave {
    interval: Duration,
    /// Session start, then the last periodic save.
    last: Instant,
}

impl PeriodicResumeSave {
    /// The interval of system `system_id` (catalog id, e.g. `"snes"`), counted from `now`.
    pub fn for_system(system_id: &str, now: Instant) -> Self {
        let large = LARGE_STATE_SYSTEMS.contains(&system_id);
        Self {
            interval: if large {
                LARGE_STATE_INTERVAL
            } else {
                INTERVAL
            },
            last: now,
        }
    }

    /// Whether to save at `now` after `real_frames`; `allowed` = the switch is on,
    /// states work and nothing else (startup load) is pending. A `true` starts the
    /// next interval even if the save then fails (no retry every frame).
    pub fn due(&mut self, now: Instant, real_frames: u64, allowed: bool) -> bool {
        let due = allowed
            && real_frames >= AUTO_SAVE_MIN_FRAMES
            && now.saturating_duration_since(self.last) >= self.interval;
        if due {
            self.last = now;
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAMES: u64 = AUTO_SAVE_MIN_FRAMES;

    #[test]
    fn saves_every_minute_and_every_three_on_large_state_systems() {
        let start = Instant::now();
        let at = |secs| start + Duration::from_secs(secs);
        let mut snes = PeriodicResumeSave::for_system("snes", start);
        assert!(!snes.due(at(59), FRAMES, true));
        assert!(snes.due(at(60), FRAMES, true));
        assert!(
            !snes.due(at(61), 1_000_000, true),
            "fast-forward frames do not count"
        );
        assert!(!snes.due(at(119), FRAMES, true));
        assert!(snes.due(at(120), FRAMES, true));
        let mut psp = PeriodicResumeSave::for_system("psp", start);
        assert!(!psp.due(at(179), FRAMES, true));
        assert!(psp.due(at(180), FRAMES, true));
    }

    #[test]
    fn nothing_is_saved_when_not_allowed_or_too_early() {
        let start = Instant::now();
        let later = start + INTERVAL;
        let mut gated = PeriodicResumeSave::for_system("snes", start);
        assert!(!gated.due(later, FRAMES, false));
        assert!(!gated.due(later, FRAMES - 1, true), "10 s of play minimum");
        assert!(
            gated.due(later, FRAMES, true),
            "a refused check does not start an interval"
        );
    }
}
