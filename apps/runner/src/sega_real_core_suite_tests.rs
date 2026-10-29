//! Mega Drive / Genesis, Master System, Game Gear and SG-1000 on Genesis Plus GX
//! (`scripts/fetch-genesis-plus-gx-core.*`) with the self-written
//! test ROMs: the full in-process check set per system, one session at a time under
//! `CORE_LOCK` (in-process reload of the core verified by the M9 phase-00 spike).
//! Skipped, with a note, when the core is missing.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use crate::run_ahead_core_tests::RunAheadExpectations;
use crate::runner_real_core_suite::{
    check_frames_and_input, core_path, load_session, repo, run_core_checks, run_state_checks,
    scratch, CORE_LOCK,
};

const CORE: &str = "genesis_plus_gx";
/// System → test ROM (GPGX picks the system by the file extension).
const TEST_ROMS: &[(&str, &str)] = &[
    ("genesis", "nostyra-test.gen"),
    ("sms", "nostyra-test.sms"),
    ("gg", "nostyra-test.gg"),
    ("sg1000", "nostyra-test.sg"),
];

/// Genesis Plus GX, measured (phase-00 spike + this suite): after an unserialize the
/// next frame yields 735 instead of 736 audio frames (the resampler's fraction is not
/// in the state) and the final state after run-ahead differs in a few bytes (all four
/// systems), while SRAM, the presented frames and a state round trip stay exact.
const GPGX_EXPECTATIONS: RunAheadExpectations = RunAheadExpectations {
    exact_state: false,
    exact_audio: false,
    exact_round_trip: true,
    state_jitter_bytes: 0,
};

/// Every system: frames / input / tone, run-ahead, slots, rewind; plus the reset
/// check (boot counter) except on the SG-1000, whose test cart has no battery RAM.
#[test]
fn runner_real_core_suite_genesis_plus_gx() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    if !core_path(CORE).exists() {
        eprintln!("skipped: run scripts/fetch-genesis-plus-gx-core first");
        return;
    }
    for &(system, file) in TEST_ROMS {
        eprintln!("genesis_plus_gx suite: {system}");
        let rom = repo(&format!("test-assets/roms/{file}"));
        let temp = scratch(&format!("core-suite-gpgx-{system}"));
        let mut session = load_session(CORE, &rom, temp.path());
        check_frames_and_input(&mut session);
        if system == "sg1000" {
            // No battery RAM on the SG-1000 test cart: no SRAM-based reset check.
            run_state_checks(&mut session, temp.path(), GPGX_EXPECTATIONS);
        } else {
            run_core_checks(&mut session, temp.path(), GPGX_EXPECTATIONS);
        }
    }
}
