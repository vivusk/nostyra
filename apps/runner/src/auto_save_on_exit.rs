//! Resume point on exit: every clean exit (menu Quit, window close, SDL Quit) saves
//! the `auto` pseudo-slot from `EmulationApp::finish()`, before the final SRAM flush,
//! so the sidecar's `sram_crc32` matches the `.srm` written right after.
//!
//! Skipped when `resume.auto_save` is off, the session has no save states (a
//! `hw_render_required` core without its context), the game never started, the loop ended
//! with an error (`finish()` also runs then; never save over a good point after a
//! core/GPU failure), or fewer than [`AUTO_SAVE_MIN_FRAMES`] real frames ran this
//! session (protects the previous point from "Start fresh, quit at once"). A failed
//! save is logged and never changes the exit code. Crash / kill → no save; the
//! previous files stay intact (atomic writes).

/// 10 s of real-timeline frames at 60 fps.
pub const AUTO_SAVE_MIN_FRAMES: u64 = 600;

/// What the loop knows when it ends.
#[derive(Debug, Clone, Copy)]
pub struct ExitFacts {
    /// `resume.auto_save`.
    pub enabled: bool,
    /// `CoreSession::states_available` (false: no state call may reach the core).
    pub states_available: bool,
    /// The game window opened (the session started).
    pub started: bool,
    /// The loop stored a fatal error.
    pub loop_failed: bool,
    /// Real-timeline frames emulated this session.
    pub real_frames: u64,
}

/// Outcome, for logs and tests.
#[derive(Debug, PartialEq, Eq)]
pub enum AutoSaveOutcome {
    Skipped(&'static str),
    Saved,
    Failed,
}

/// Why the resume point must not be written, if it must not.
pub fn skip_reason(facts: ExitFacts) -> Option<&'static str> {
    if !facts.enabled {
        Some("resume.auto_save is off")
    } else if !facts.states_available {
        Some("save states need hardware rendering")
    } else if !facts.started {
        Some("game never started")
    } else if facts.loop_failed {
        Some("session ended with an error")
    } else if facts.real_frames < AUTO_SAVE_MIN_FRAMES {
        Some("session shorter than 10 s")
    } else {
        None
    }
}

/// Runs `save` when the policy allows; errors are logged only (callers ignore the
/// outcome, so quitting is never blocked or turned into a failure).
pub fn auto_save_on_exit(
    facts: ExitFacts,
    save: impl FnOnce() -> anyhow::Result<()>,
) -> AutoSaveOutcome {
    if let Some(reason) = skip_reason(facts) {
        tracing::info!(reason, frames = facts.real_frames, "resume point not saved");
        return AutoSaveOutcome::Skipped(reason);
    }
    match save() {
        Ok(()) => {
            tracing::info!(frames = facts.real_frames, "resume point saved");
            AutoSaveOutcome::Saved
        }
        Err(err) => {
            tracing::warn!("resume point save failed: {err:#}");
            AutoSaveOutcome::Failed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: ExitFacts = ExitFacts {
        enabled: true,
        states_available: true,
        started: true,
        loop_failed: false,
        real_frames: AUTO_SAVE_MIN_FRAMES,
    };

    fn outcome(facts: ExitFacts) -> (AutoSaveOutcome, bool) {
        let mut called = false;
        let outcome = auto_save_on_exit(facts, || {
            called = true;
            Ok(())
        });
        (outcome, called)
    }

    #[test]
    fn saves_after_ten_seconds_of_clean_play() {
        assert_eq!(outcome(GOOD), (AutoSaveOutcome::Saved, true));
        let long = ExitFacts {
            real_frames: 1_000_000,
            ..GOOD
        };
        assert_eq!(outcome(long), (AutoSaveOutcome::Saved, true));
    }

    #[test]
    fn every_skip_rule_leaves_the_previous_point_alone() {
        let cases = [
            ExitFacts {
                enabled: false,
                ..GOOD
            },
            ExitFacts {
                states_available: false,
                ..GOOD
            },
            ExitFacts {
                started: false,
                ..GOOD
            },
            ExitFacts {
                loop_failed: true,
                ..GOOD
            },
            ExitFacts {
                real_frames: AUTO_SAVE_MIN_FRAMES - 1,
                ..GOOD
            },
            ExitFacts {
                real_frames: 0,
                ..GOOD
            },
        ];
        for facts in cases {
            let (result, called) = outcome(facts);
            assert!(matches!(result, AutoSaveOutcome::Skipped(_)), "{facts:?}");
            assert!(!called, "save must not run: {facts:?}");
        }
    }

    #[test]
    fn save_error_is_reported_not_propagated() {
        let result = auto_save_on_exit(GOOD, || anyhow::bail!("disk full"));
        assert_eq!(result, AutoSaveOutcome::Failed);
    }
}
