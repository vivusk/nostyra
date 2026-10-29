//! Every runner check that needs a real core + test ROM (fetch cores with
//! `scripts/fetch-snes9x-core.*` / `fetch-mesen-core.*`; mgba in
//! `handheld_real_core_suite_tests`). One suite test fn per core:
//! a process hosts one core session at a time (`CORE_LOCK`), and the harness would
//! otherwise run tests on parallel threads. Each check starts from whatever state the
//! previous one left and resets what it relies on itself.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use libretro_host::{CoreSession, NullAudioSink, NullVideoSink};
use system_catalog::SaveStateQuality;

pub(crate) use crate::real_core_session_loader::{core_path, load_session, load_session_with};
use crate::run_ahead_core_tests::{RunAheadExpectations, EXACT_RUN_AHEAD};
use crate::runner_held_input::HeldInput;
use crate::{
    reset_core_tests, rewind_core_tests, run_ahead_core_tests, save_state_slots_core_tests,
    state_policy_core_tests,
};

pub(crate) fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// Serialises core sessions in this binary: the suites and the measurement test each
/// load their own session and must never overlap (e.g. with `--include-ignored`).
pub(crate) static CORE_LOCK: Mutex<()> = Mutex::new(());

/// Fresh temp dir for a session's saves; removed on drop. Bind it BEFORE the session
/// so it drops after it: on Windows the loaded core keeps files in it open.
pub(crate) fn scratch(tag: &str) -> tempfile::TempDir {
    let prefix = format!("nostyra-{tag}-");
    tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
}

/// The in-process checks every core passes.
pub(crate) fn run_core_checks(
    session: &mut CoreSession,
    dir: &Path,
    run_ahead: RunAheadExpectations,
) {
    run_state_checks(session, dir, run_ahead);
    reset_core_tests::check_reset(session);
}

/// Run-ahead, state slots and rewind (cores whose test content has no battery
/// save counter skip the SRAM-based reset check).
pub(crate) fn run_state_checks(
    session: &mut CoreSession,
    dir: &Path,
    run_ahead: RunAheadExpectations,
) {
    run_ahead_core_tests::check_run_ahead(session, run_ahead);
    save_state_slots_core_tests::check_state_slots(session, dir, true);
    rewind_core_tests::check_rewind(session, run_ahead);
}

#[test]
fn runner_real_core_suite() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let rom = repo("test-assets/roms/nostyra-test.sfc");
    let temp = scratch("core-suite"); // slot states are ~65 MiB each
    let mut session = load_session("snes9x", &rom, temp.path());
    run_core_checks(&mut session, temp.path(), EXACT_RUN_AHEAD);
    // Same session, quality forced: the refusal path N64 takes.
    state_policy_core_tests::check_states_refused(&mut session, SaveStateQuality::Serialized);
}

/// NES on Mesen with `nostyra-test.nes`; skipped (with a note) when the core is absent.
#[test]
fn runner_real_core_suite_mesen() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    if !core_path("mesen").exists() {
        eprintln!("skipped: run scripts/fetch-mesen-core first");
        return;
    }
    let rom = repo("test-assets/roms/nostyra-test.nes");
    let temp = scratch("core-suite-mesen");
    let mut session = load_session("mesen", &rom, temp.path());
    check_frames_and_input(&mut session);
    let mesen_quirks = RunAheadExpectations {
        exact_state: false,
        exact_audio: false,
        exact_round_trip: true,
        state_jitter_bytes: 0,
    };
    run_core_checks(&mut session, temp.path(), mesen_quirks);
}

/// Same start state twice → same frame hashes; holding a button changes the frame;
/// the ROM's square wave makes non-silent audio.
pub(crate) fn check_frames_and_input(session: &mut CoreSession) {
    // Past boot first: a state taken before the first `retro_run` restores without
    // the core's first-frame setup, so its first frame differs.
    let (mut video, mut audio) = (NullVideoSink::default(), NullAudioSink::default());
    for _ in 0..10 {
        session
            .run_frame(&mut video, &mut audio, &mut HeldInput::None)
            .unwrap();
    }
    let start = session.serialize().unwrap();
    let frames = |session: &mut CoreSession, held: HeldInput| {
        session.unserialize(&start).unwrap();
        let mut video = NullVideoSink::default();
        let mut audio = NullAudioSink::default();
        let mut input = held;
        (0..30)
            .map(|_| {
                session
                    .run_frame(&mut video, &mut audio, &mut input)
                    .unwrap();
                video.last_checksum
            })
            .collect::<Vec<_>>()
    };
    let plain = frames(session, HeldInput::None);
    assert_eq!(
        plain,
        frames(session, HeldInput::None),
        "deterministic frames"
    );
    assert_ne!(
        plain,
        frames(session, HeldInput::B),
        "input changes the frame"
    );
    session.unserialize(&start).unwrap();
    let mut sound = LoudestSample(0);
    for _ in 0..10 {
        session
            .run_frame(&mut video, &mut sound, &mut HeldInput::None)
            .unwrap();
    }
    assert!(sound.0 > 1000, "square wave is audible (peak {})", sound.0);
    session.unserialize(&start).unwrap();
}

/// Peak absolute sample seen.
pub(crate) struct LoudestSample(pub(crate) i32);

impl libretro_host::AudioSink for LoudestSample {
    fn samples(&mut self, interleaved: &[i16]) {
        let peak = interleaved.iter().map(|&s| i32::from(s).abs()).max();
        self.0 = self.0.max(peak.unwrap_or(0));
    }
}

/// Rewind history per 64 MiB (phase-04 measurement). Test ROM by default; set
/// `NOSTYRA_REWIND_ROM=<path to a local .sfc>` to measure a real game (never commit
/// ROMs). `cargo test -p nostyra-runner --release rewind_history -- --ignored --nocapture`
#[test]
#[ignore = "measurement; run with --release -- --ignored --nocapture"]
fn rewind_history_measurement() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let rom = std::env::var_os("NOSTYRA_REWIND_ROM")
        .map_or_else(|| repo("test-assets/roms/nostyra-test.sfc"), PathBuf::from);
    let temp = scratch("rewind-measure");
    let mut session = load_session("snes9x", &rom, temp.path());
    let label = rom.file_name().unwrap_or_default().to_string_lossy();
    rewind_core_tests::measure_history(&mut session, &label, 60);
}
