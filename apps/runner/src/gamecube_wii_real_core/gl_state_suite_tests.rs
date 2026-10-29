//! Dolphin save states on the OpenGL renderer (behind `gl_test_gate`; under Null or
//! Software any state call segfaults). Through the runner's own paths: slot round trip
//! through the guard (MEM1 restored), truncated / CRC-broken / other-version (strict)
//! / other-disc (content tag) states refused before the core, startup resume of
//! `auto` right after the load (existing re-query hold; loadable from frame 1), in
//! several sessions in a row. The last session runs on its own thread, which must
//! exit: `FreeLibrary` after an unserialize would hang it (Dolphin is kept loaded).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;
use std::sync::PoisonError;
use std::time::{Duration, Instant};

use ipc_protocol::{StateSidecar, StateSlotId};
use libretro_host::CoreSession;
use system_catalog::{CoreSpec, GC};

use super::checks::{boot, run};
use super::{debug_word, dolphin, load_dol, options, system_folder, FIELDS, GC_DOL};
use crate::game_save_data::GameSaveData;
use crate::real_core_session_loader::gl_test_gate;
use crate::runner_data_paths::RunnerDataPaths;
use crate::runner_held_input::HeldInput;
use crate::runner_real_core_suite::{repo, scratch, CORE_LOCK};
use crate::save_state_slots::SaveStateSlots;
use crate::startup_state_load::{StartupStateLoad, MAX_RETRY_FRAMES, RESUMED};

/// The session's disc tag (a DOL has none; set as for a disc so the tag check runs).
const TAG: &str = "NSTY01:0:0";
const SLOT: StateSlotId = StateSlotId::Numbered(1);
/// Longest wait for the last session's thread to exit after its session dropped.
const THREAD_EXIT: Duration = Duration::from_secs(30);

#[test]
fn gamecube_wii_gl_state_suite_dolphin() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(core) = dolphin() else { return };
    if !gl_test_gate() {
        return;
    }
    let system = system_folder(core);
    let provider = hw_render_gl::SdlGlProvider::new();
    let data = scratch("core-suite-dolphin-states");
    let mut paths = RunnerDataPaths::resolve(Some(data.path()), &repo(GC_DOL), &GC, None).unwrap();
    paths.state_content_tag = Some(TAG.into());
    let auto_fields = {
        eprintln!("dolphin gl state suite: slots and guard");
        let mut session = gl_session(core, system.path(), &paths, &provider);
        boot(&mut session);
        check_slot_round_trip_and_guard(&mut session, &paths);
        run(&mut session, HeldInput::None, 20);
        let started = Instant::now();
        SaveStateSlots::for_game(&paths)
            .save(&mut session, StateSlotId::Auto, None)
            .unwrap();
        eprintln!(
            "dolphin gl state suite: auto saved in {:?}",
            started.elapsed()
        );
        debug_word(&mut session, FIELDS)
    };
    for round in 1..=2 {
        eprintln!("dolphin gl state suite: startup resume {round}");
        let mut session = gl_session(core, system.path(), &paths, &provider);
        check_startup_resume(&mut session, &paths, auto_fields);
    }
    eprintln!("dolphin gl state suite: startup resume on a thread that then exits");
    let (system_dir, thread_paths) = (system.path().to_path_buf(), paths.clone());
    let handle = std::thread::spawn(move || {
        let provider = hw_render_gl::SdlGlProvider::new();
        let mut session = gl_session(core, &system_dir, &thread_paths, &provider);
        check_startup_resume(&mut session, &thread_paths, auto_fields);
    });
    let started = Instant::now();
    while !handle.is_finished() {
        assert!(
            started.elapsed() < THREAD_EXIT,
            "session thread exits (keep-loaded)"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    handle.join().unwrap();
}

/// A Hardware-renderer session on the game's save folder; `provider` outlives it.
fn gl_session(
    core: &CoreSpec,
    system: &Path,
    paths: &RunnerDataPaths,
    provider: &hw_render_gl::SdlGlProvider,
) -> CoreSession {
    let dirs = (system, paths.saves_dir.as_path());
    let session = load_dol(core, GC_DOL, dirs, &options(&GC, &[]), Some(provider));
    assert!(session.uses_hw_context() && session.states_available());
    session
}

fn sidecar(paths: &RunnerDataPaths) -> StateSidecar {
    serde_json::from_slice(&std::fs::read(paths.states_dir.join("slot1.json")).unwrap()).unwrap()
}

fn refusal(slots: &SaveStateSlots, session: &mut CoreSession) -> String {
    format!("{:#}", slots.load(session, SLOT).expect_err("refused"))
}

/// Save, run on, load: MEM1 back at the saved field count; then every refusal.
fn check_slot_round_trip_and_guard(session: &mut CoreSession, paths: &RunnerDataPaths) {
    let slots = SaveStateSlots::for_game(paths);
    let started = Instant::now();
    slots.save(session, SLOT, None).unwrap();
    let saved_fields = debug_word(session, FIELDS);
    let saved = sidecar(paths);
    eprintln!(
        "dolphin gl state suite: {} bytes saved in {:?}",
        saved.state_len.unwrap(),
        started.elapsed()
    );
    assert_eq!(saved.content_tag.as_deref(), Some(TAG));
    run(session, HeldInput::None, 30);
    assert!(debug_word(session, FIELDS) > saved_fields + 20);
    slots.load(session, SLOT).unwrap();
    assert_eq!(debug_word(session, FIELDS), saved_fields, "MEM1 restored");
    run(session, HeldInput::None, 5);
    assert!(
        debug_word(session, FIELDS) > saved_fields,
        "runs on after the load"
    );

    let state = paths.states_dir.join("slot1.state");
    let good = std::fs::read(&state).unwrap();
    let mut flipped = good.clone();
    flipped[good.len() / 2] ^= 0xFF;
    for bad in [&flipped[..], &good[..good.len() - 1]] {
        std::fs::write(&state, bad).unwrap();
        let text = refusal(&slots, session);
        assert!(text.contains("damaged"), "{text}");
    }
    std::fs::write(&state, &good).unwrap();
    let json = paths.states_dir.join("slot1.json");
    let refused_with = |session: &mut CoreSession, sidecar: &StateSidecar| {
        std::fs::write(&json, serde_json::to_vec(sidecar).unwrap()).unwrap();
        refusal(&slots, session)
    };
    let version = StateSidecar {
        core_version: "2512.0.1+0".into(),
        ..saved.clone()
    };
    let text = refused_with(session, &version);
    assert_eq!(text, "state was made by another Dolphin version");
    let disc2 = StateSidecar {
        content_tag: Some("NSTY01:1:0".into()),
        ..saved.clone()
    };
    let text = refused_with(session, &disc2);
    assert_eq!(text, "state was made from another disc or revision");
    std::fs::write(&json, serde_json::to_vec(&saved).unwrap()).unwrap();
    slots.load(session, SLOT).unwrap();
    assert_eq!(debug_word(session, FIELDS), saved_fields, "loads again");
}

/// `--load-state auto` as the runner does it: attempts before the first frame and
/// after each one until answered; resumes at the saved field count `auto_fields`.
fn check_startup_resume(session: &mut CoreSession, paths: &RunnerDataPaths, auto_fields: u32) {
    let saves = GameSaveData::load(paths, session, 0);
    let quality = crate::runner_core_options::savestate_quality(&GC);
    let mut load = StartupStateLoad::new(StateSlotId::Auto, quality);
    let mut frames = 0;
    let outcome = loop {
        if let Some(outcome) = load.attempt(&saves, session) {
            break outcome;
        }
        assert!(
            frames <= MAX_RETRY_FRAMES,
            "answered within the retry window"
        );
        run(session, HeldInput::None, 1);
        frames += 1;
    };
    eprintln!("dolphin gl state suite: resume answered after {frames} frames");
    assert!(outcome.loaded, "{}", outcome.status);
    assert_eq!(outcome.status, RESUMED);
    assert_eq!(
        debug_word(session, FIELDS),
        auto_fields,
        "MEM1 of the resume point"
    );
    run(session, HeldInput::None, 10);
    assert!(
        debug_word(session, FIELDS) > auto_fields,
        "runs on after the resume"
    );
}
