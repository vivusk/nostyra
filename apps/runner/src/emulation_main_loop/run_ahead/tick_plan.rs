//! Pure step planner for one tick: which `retro_run` calls happen, in which role.
//! Run-ahead applies only to the last due frame, so catch-up ticks stay cheap.

/// One `retro_run` of a tick, with its role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FrameStep {
    /// Run-ahead off: normal frame, full AV.
    Plain,
    /// Catch-up frame on the real timeline: audio only, no state taken.
    CatchUp,
    /// Real frame whose end state is saved for the rollback.
    Real,
    /// Hidden frame ahead of the real timeline: no AV.
    Hidden,
    /// Last frame ahead: its picture is presented, then the state is rolled back.
    Presented,
}

impl FrameStep {
    /// Frames on the real timeline: they feed real audio, pacing and SRAM flushes.
    pub(crate) fn is_real_timeline(self) -> bool {
        matches!(self, Self::Plain | Self::CatchUp | Self::Real)
    }
}

/// Steps for one tick with `due` frames due (no allocation).
/// e.g. due 2, run-ahead 2 → CatchUp, Real, Hidden, Presented.
pub(crate) fn plan_tick(due: u32, run_ahead: u8) -> impl Iterator<Item = FrameStep> {
    let (plain, catch_up, ahead) = match (due, run_ahead) {
        (0, _) => (0, 0, 0),
        (due, 0) => (due, 0, 0),
        (due, n) => (0, due - 1, u32::from(n)),
    };
    let real = u32::from(ahead > 0);
    let hidden = ahead.saturating_sub(1);
    let presented = u32::from(ahead > 0);
    let repeat = |step, n| std::iter::repeat_n(step, n as usize);
    repeat(FrameStep::Plain, plain)
        .chain(repeat(FrameStep::CatchUp, catch_up))
        .chain(repeat(FrameStep::Real, real))
        .chain(repeat(FrameStep::Hidden, hidden))
        .chain(repeat(FrameStep::Presented, presented))
}

#[cfg(test)]
mod tests {
    use super::*;
    use FrameStep::*;

    fn plan(due: u32, run_ahead: u8) -> Vec<FrameStep> {
        plan_tick(due, run_ahead).collect()
    }

    #[test]
    fn run_ahead_off_runs_plain_frames() {
        assert_eq!(plan(0, 0), []);
        assert_eq!(plan(2, 0), [Plain, Plain]);
    }

    #[test]
    fn run_ahead_only_on_the_last_due_frame() {
        assert_eq!(plan(1, 1), [Real, Presented]);
        assert_eq!(plan(1, 2), [Real, Hidden, Presented]);
        // Catch-up: RA 2 × 2 due = 4 retro_run, not 6.
        assert_eq!(plan(2, 2), [CatchUp, Real, Hidden, Presented]);
        assert_eq!(plan(0, 2), []);
    }

    #[test]
    fn only_real_timeline_frames_feed_audio() {
        let real: Vec<bool> = plan(2, 2)
            .into_iter()
            .map(FrameStep::is_real_timeline)
            .collect();
        assert_eq!(real, [true, true, false, false]);
    }
}
