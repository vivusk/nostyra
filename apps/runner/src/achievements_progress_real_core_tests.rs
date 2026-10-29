//! Achievement progress through a real slot state (M15 phase 07): Snes9x + the SNES
//! test ROM, a set whose achievement needs 25 hits of "frame counter went up"
//! (`>d`, so it arms on the load frame). Session A saves slot 0 after 10 hits with its
//! progress in the sidecar; session B (a later launch) loads the slot and restores the
//! progress → unlock 15 frames later; session C loads the same slot without progress →
//! progress resets, no unlock by then.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use ipc_protocol::RunnerEvent;

use crate::achievement_session::{self, progress_of};
use crate::achievements_fake_set_session::{
    start_fake_set_session, CoreFacts, FakeSetSession, RaCase,
};
use crate::achievements_real_core_loader::{load_ra_core, run_quiet_frame, RaCoreSession};
use crate::runner_real_core_suite::{scratch, CORE_LOCK};
use crate::save_state_slots::{SaveStateSlots, StateSlotId};

const HITS_ACH: u32 = 77;
const SLOT: StateSlotId = StateSlotId::Numbered(0);

/// Session over the SNES case with the 25-hit set, after its load frame.
fn hit_session(case: &RaCase, core: &mut RaCoreSession, data: &std::path::Path) -> FakeSetSession {
    let info = core.session.core_info().clone();
    let facts = CoreFacts {
        library_name: &info.name,
        version: &info.version,
        options: &core.options,
    };
    let mut started = start_fake_set_session(case, &case.rom(), data, false, &facts);
    let counter = format!("0xH{:06x}", case.counter_address);
    let condition = format!("{counter}>d{counter}.25.");
    let set =
        achievements::fake_game_set_json(case.game_id, "Hits", &[(HITS_ACH, &condition)], &[]);
    started.fake.respond("achievementsets", 200, &set);
    frames(core, &mut started, 1); // set loads (regions ready) and arms
    started
}

/// Runs `n` frames; returns the frame (1-based) the hits achievement unlocked on.
fn frames(core: &mut RaCoreSession, started: &mut FakeSetSession, n: u32) -> Option<u32> {
    let mut unlocked_at = None;
    for frame in 1..=n {
        run_quiet_frame(&mut core.session);
        achievement_session::after_real_frame(&mut started.session, &core.session);
        let unlocked =
            started.events.borrow().iter().any(
                |e| matches!(e, RunnerEvent::AchievementUnlocked { id, .. } if *id == HITS_ACH),
            );
        if unlocked && unlocked_at.is_none() {
            unlocked_at = Some(frame);
        }
    }
    unlocked_at
}

#[test]
fn slot_state_progress_round_trips_and_a_state_without_it_resets() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let case = RaCase::load("snes");
    let mut core = match load_ra_core("snes", &case.rom(), None) {
        Ok(core) => core,
        Err(reason) => return eprintln!("skipped ({reason})"),
    };
    let (data, states) = (scratch("ra-progress-data"), scratch("ra-progress-states"));
    let slots = SaveStateSlots::new(states.path().to_path_buf(), u64::MAX);

    let mut a = hit_session(&case, &mut core, data.path());
    assert_eq!(frames(&mut core, &mut a, 10), None);
    let saved = progress_of(&a.session).expect("progress while loaded");
    a.session.take().unwrap().shutdown();
    slots
        .save(&mut core.session, SLOT, Some(saved.clone()))
        .unwrap();

    let mut b = hit_session(&case, &mut core, data.path());
    let progress = slots.load(&mut core.session, SLOT).unwrap();
    assert_eq!(
        progress.as_deref(),
        Some(saved.as_str()),
        "sidecar carries it"
    );
    let b_session = b.session.as_mut().unwrap();
    b_session.apply_loaded_progress(progress.as_deref());
    let restored = frames(&mut core, &mut b, 30);
    assert_eq!(restored, Some(15), "10 restored hits + 15");
    b.session.take().unwrap().shutdown();

    let mut c = hit_session(&case, &mut core, data.path());
    slots.load(&mut core.session, SLOT).unwrap();
    c.session.as_mut().unwrap().apply_loaded_progress(None);
    assert_eq!(frames(&mut core, &mut c, 20), None, "hits reset");
    c.session.take().unwrap().shutdown();
}
