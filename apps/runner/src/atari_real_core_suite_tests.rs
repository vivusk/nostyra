//! Atari 2600 (Stella), 7800 (ProSystem) and Lynx (Handy) with the self-written test
//! ROMs (`test-assets/roms/src/build-nostyra-test-atari-roms.py`): frames / input /
//! tone, run-ahead, slots and rewind per system, plus the facts the M17 spike pinned
//! (ProSystem fast-context size, 2600 console switch, `.a78` = headerless `.bin`,
//! Lynx header rotation). No SRAM on these carts: no boot-counter reset check. Each
//! core is skipped with a note when it is missing.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use libretro_host::{CoreSession, NullAudioSink, NullVideoSink, SavestateContext};

use crate::run_ahead_core_tests::{check_run_ahead, RunAheadExpectations};
use crate::runner_held_input::HeldInput;
use crate::runner_real_core_suite::{
    check_frames_and_input_from, core_path, load_session, repo, scratch, CORE_LOCK,
};
use crate::{rewind_core_tests, save_state_slots_core_tests};

/// Measured in the M17 spike: no core keeps the final state or audio byte-exact under
/// run-ahead; Stella re-computes a few bytes on its first load; Stella and Handy show
/// one stale frame right after a load.
fn expectations(core: &str) -> RunAheadExpectations {
    RunAheadExpectations {
        exact_state: false,
        exact_audio: false,
        exact_round_trip: core != "stella",
        state_jitter_bytes: 0,
        stale_first_frame: core != "prosystem",
    }
}

/// (core, test ROM) per system.
const ROMS: &[(&str, &str)] = &[
    ("stella", "nostyra-test.a26"),
    ("prosystem", "nostyra-test.a78"),
    ("prosystem", "nostyra-test-7800.bin"),
    ("handy", "nostyra-test.o"),
    ("handy", "nostyra-test.lnx"),
];

fn frame_after(session: &mut CoreSession, held: HeldInput, frames: u32) -> u64 {
    let (mut video, mut audio, mut input) =
        (NullVideoSink::default(), NullAudioSink::default(), held);
    for _ in 0..frames {
        session
            .run_frame(&mut video, &mut audio, &mut input)
            .unwrap();
    }
    video.last_checksum
}

#[test]
fn runner_real_core_suite_atari() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    for &(core, file) in ROMS {
        if !core_path(core).exists() {
            eprintln!("skipped {file}: run scripts/fetch-{core}-core first");
            continue;
        }
        eprintln!("atari suite: {core} {file}");
        let temp = scratch(&format!("core-suite-{core}"));
        let mut session = load_session(
            core,
            &repo(&format!("test-assets/roms/{file}")),
            temp.path(),
        );
        if file == "nostyra-test.lnx" {
            // Before any input: the scripted run-ahead input presses SELECT (rotate).
            let geometry = session.av_info().geometry;
            assert_eq!(
                (geometry.base_width, geometry.base_height),
                (102, 160),
                "rotated"
            );
        }
        let expect = expectations(core);
        check_frames_and_input_from(&mut session, usize::from(expect.stale_first_frame));
        check_run_ahead(&mut session, expect);
        let exact_replay = !expect.stale_first_frame;
        save_state_slots_core_tests::check_state_slots(&mut session, temp.path(), exact_replay);
        rewind_core_tests::check_rewind(&mut session, expect);
        match file {
            "nostyra-test.a26" => check_console_reset_switch(&mut session),
            "nostyra-test.a78" | "nostyra-test-7800.bin" => check_fast_context_size(&mut session),
            _ => {}
        }
    }
}

/// START = Game Reset (SWCHB bit 0): the 2600 test ROM shows its reset hue.
fn check_console_reset_switch(session: &mut CoreSession) {
    let start = session.serialize().unwrap();
    let plain = frame_after(session, HeldInput::None, 5);
    session.unserialize(&start).unwrap();
    let reset = frame_after(session, HeldInput::Joypad(3), 5);
    assert_ne!(plain, reset, "START reaches SWCHB");
    session.unserialize(&start).unwrap();
}

/// ProSystem sizes its state by the savestate context: 83 968 B in the fast one.
fn check_fast_context_size(session: &mut CoreSession) {
    let fast = SavestateContext::RunaheadSameInstance;
    let mut state = Vec::new();
    assert_eq!(session.serialize_into(&mut state, fast).unwrap(), 83_968);
    session.unserialize_from(&state, fast).unwrap();
    assert_eq!(session.serialize().unwrap().len(), 49_221, "Normal context");
}
