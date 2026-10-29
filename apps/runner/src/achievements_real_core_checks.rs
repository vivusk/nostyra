//! One RA system's achievement mode on its real core (M15 phase 07): a fake-set session
//! fed through the `after_real_frame` hook the headless runner uses, and an independent
//! probe of the RA memory regions that tells on which frame the frame counter reaches
//! the unlock value.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use achievements::{form_field, request_kind, LazyMemoryRegions, RegionStatus};
use ipc_protocol::RunnerEvent;

use crate::achievement_session;
use crate::achievements_fake_set_session::{start_fake_set_session, CoreFacts, RaCase};
use crate::achievements_real_core_loader::{run_quiet_frame, RaCoreSession};

/// What one achievement session saw (frames count from 1 after its start).
#[derive(Debug, Default)]
pub(crate) struct ModeRun {
    /// First frame the probe's regions were ready (the set request starts there).
    pub(crate) regions_ready_at: Option<u32>,
    pub(crate) loaded_at: Option<u32>,
    pub(crate) unlocked_at: Option<u32>,
    /// First frame after the load whose counter equals the unlock value.
    pub(crate) counter_hit_at: Option<u32>,
    pub(crate) events: Vec<RunnerEvent>,
    pub(crate) notices: Vec<String>,
    pub(crate) posts: Vec<String>,
}

impl ModeRun {
    pub(crate) fn posts_of(&self, kind: &str) -> Vec<&String> {
        self.posts
            .iter()
            .filter(|post| request_kind(post) == kind)
            .collect()
    }

    pub(crate) fn field_of(&self, kind: &str, key: &str) -> Option<String> {
        let post = self.posts_of(kind).into_iter().next()?;
        form_field(post, key).map(str::to_owned)
    }
}

/// Starts a session for `case` on the running `core` (hardcore with the hidden UI flag
/// forced on), runs up to `max_frames` until the unlock (and in hardcore the leaderboard
/// submit) happened, then shuts the session down as the runner does.
pub(crate) fn run_mode(
    case: &RaCase,
    core: &mut RaCoreSession,
    data: &Path,
    hardcore: bool,
    max_frames: u32,
) -> ModeRun {
    let info = core.session.core_info().clone();
    let facts = CoreFacts {
        library_name: &info.name,
        version: &info.version,
        options: &core.options,
    };
    let started = start_fake_set_session(case, &case.rom(), data, hardcore, &facts);
    let (fake, events) = (started.fake, started.events);
    let mut achievements = started.session;
    let mut probe = LazyMemoryRegions::new(case.console_id);
    let mut run = ModeRun::default();
    for frame in 1..=max_frames {
        run_quiet_frame(&mut core.session);
        achievement_session::after_real_frame(&mut achievements, &core.session);
        let session = &core.session;
        let status = probe.after_real_frame(session.memory_maps(), &|id| session.memory_info(id));
        if status == RegionStatus::Ready {
            run.regions_ready_at.get_or_insert(frame);
        }
        let value = counter(&probe, case);
        record_frame(
            &mut run,
            frame,
            &events.borrow(),
            (value, case.unlock_value),
        );
        run.notices
            .extend(achievements.as_mut().unwrap().take_notices());
        let submitted = !hardcore || fake.kinds().iter().any(|k| k == "submitlbentry");
        if run.unlocked_at.is_some() && submitted {
            break;
        }
    }
    achievements.take().unwrap().shutdown();
    run.events = events.borrow().clone();
    run.posts = fake.posts();
    run
}

fn counter(probe: &LazyMemoryRegions, case: &RaCase) -> Option<u8> {
    let mut byte = [0u8; 1];
    (probe.read(case.counter_address, &mut byte) == 1).then_some(byte[0])
}

fn record_frame(
    run: &mut ModeRun,
    frame: u32,
    events: &[RunnerEvent],
    (counter, unlock): (Option<u8>, u8),
) {
    let loaded = |e: &RunnerEvent| matches!(e, RunnerEvent::AchievementsLoaded { .. });
    let unlocked = |e: &RunnerEvent| matches!(e, RunnerEvent::AchievementUnlocked { .. });
    if run.loaded_at.is_none() && events.iter().any(loaded) {
        run.loaded_at = Some(frame);
    } else if run.loaded_at.is_some() && run.counter_hit_at.is_none() {
        // Armed only after a false evaluation: the load frame itself never counts.
        if counter == Some(unlock) {
            run.counter_hit_at = Some(frame);
        }
    }
    if run.unlocked_at.is_none() && events.iter().any(unlocked) {
        run.unlocked_at = Some(frame);
    }
}
