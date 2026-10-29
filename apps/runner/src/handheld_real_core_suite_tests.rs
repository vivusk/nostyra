//! Game Boy, Game Boy Color and Game Boy Advance on mGBA (`scripts/fetch-mgba-core.*`)
//! with the self-written test ROMs: the full in-process check set per system, one
//! session at a time under `CORE_LOCK` (in-process reload of the core verified by the
//! M7 phase-00 spike). Skipped, with a note, when the core is missing. The Nintendo
//! DS suite (melonDS DS) is registered here as a submodule.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use crate::run_ahead_core_tests::RunAheadExpectations;
use crate::runner_real_core_suite::{
    check_frames_and_input, core_path, load_session, repo, run_core_checks, scratch, CORE_LOCK,
};

#[path = "nds_real_core_suite_tests.rs"]
mod nds_real_core_suite_tests;

/// mGBA, measured (phase-00 spike, fresh session per check + this suite): under
/// run-ahead the final state and the audio stream differ (GB/GBC: the stream is 1024
/// samples short once) while SRAM and the presented frames stay exact, so those two
/// checks relax as for Mesen. GBA mode also rewrites 4 state bytes on the first load
/// of a state (stable after), so only GBA relaxes the rewind round-trip.
fn mgba_expectations(system: &str) -> RunAheadExpectations {
    RunAheadExpectations {
        exact_state: false,
        exact_audio: false,
        exact_round_trip: system != "gba",
        state_jitter_bytes: 0,
    }
}

#[test]
fn runner_real_core_suite_mgba() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    if !core_path("mgba").exists() {
        eprintln!("skipped: run scripts/fetch-mgba-core first");
        return;
    }
    for system in ["gb", "gbc", "gba"] {
        eprintln!("mgba suite: {system}");
        let rom = repo(&format!("test-assets/roms/nostyra-test.{system}"));
        let temp = scratch(&format!("core-suite-mgba-{system}"));
        let mut session = load_session("mgba", &rom, temp.path());
        check_frames_and_input(&mut session);
        run_core_checks(&mut session, temp.path(), mgba_expectations(system));
    }
}
