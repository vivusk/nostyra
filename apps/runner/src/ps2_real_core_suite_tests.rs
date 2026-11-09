//! Sony PlayStation 2 on PCEE2 (`scripts/fetch-pcee2-core.*`) with the self-written
//! test disc (`test-assets/roms/src/build-nostyra-test-ps2.py`). No HLE BIOS: runs
//! only where `NOSTYRA_PS2_BIOS_DIR` names the user's dump (`bios_gate` copies it into
//! the scratch system folder, never into the repo), else skips with a note like the
//! missing-DLL skips. OpenGL on the host context when `gl_test_gate` passes (the
//! runner's path), else the core's own readback device (same picture, spike step 5).
//! The program drives only the GS: no input / audio assertions
//! (`check_frames_and_input` is not used) — picture + RAM counter instead. Several
//! PCEE2 sessions per process are fine (spike step 11): shared binary, `CORE_LOCK`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;
use std::time::Instant;

use ipc_protocol::{StateSidecar, StateSlotId};
use libretro_host::{CoreSession, HwRenderProvider};
use system_catalog::{DEFAULT_MAX_STATE_BYTES, PS2};

use crate::achievements_real_core_loader::ps2_bios_gate as bios_gate;
use crate::bios_option_switch::{launch_option, ps2_bios_option::tests::memo_lock};
use crate::game_save_data::GameSaveData;
use crate::real_core_session_loader::{gl_test_gate, load_session_in};
use crate::runner_core_options;
use crate::runner_data_paths::RunnerDataPaths;
use crate::runner_real_core_suite::{core_path, repo, scratch, CORE_LOCK};
use crate::save_state_slots::SaveStateSlots;
use crate::startup_state_load::{StartupStateLoad, MAX_RETRY_FRAMES, RESUMED};
use crate::state_load_checks::check_state_size;
use crate::state_policy_core_tests;
use checks::{
    boot, check_core_files, check_counter_advances, check_picture, check_reset_reboots, counter,
    run,
};

#[path = "ps2_real_core_checks_tests.rs"]
mod checks;

const ISO: &str = "test-assets/roms/nostyra-test-ps2.iso";
const SLOT: StateSlotId = StateSlotId::Numbered(1);

#[test]
fn ps2_suite_pcee2() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let core = system_catalog::core_for_system(PS2.id).unwrap();
    if !core_path(core.id).exists() {
        eprintln!("skipped: run scripts/fetch-{}-core first", core.id);
        return;
    }
    let data = scratch("core-suite-ps2");
    let iso = repo(ISO); // native separators: PCEE2 refuses `E:/…` (spike D7)
    let paths = RunnerDataPaths::resolve(Some(data.path()), &iso, &PS2, None).unwrap();
    let stems = match bios_gate::install_user_bios(&paths.system_dir) {
        Ok(stems) => stems,
        Err(reason) => return eprintln!("skipped: {reason}"),
    };
    std::fs::create_dir_all(&paths.saves_dir).unwrap();
    let provider = gl_test_gate().then(hw_render_gl::SdlGlProvider::new);
    let gl = provider.as_ref().map(|p| p as &dyn HwRenderProvider);
    let options = {
        let _memo = memo_lock();
        let bios = launch_option(&PS2, &paths.system_dir, &iso).expect("picker answers the BIOS");
        runner_core_options::resolve(&PS2, &[bios])
    };
    let dirs = (paths.system_dir.as_path(), paths.saves_dir.as_path());
    let load = || load_session_in(core.id, &iso, dirs, &options, gl);
    let auto_counter = {
        eprintln!("pcee2 suite: boot, picture, counter, states, reset");
        let mut session = load();
        let frames = boot(&mut session);
        eprintln!(
            "pcee2 suite: ELF running after {frames} frames (hw context: {})",
            session.uses_hw_context()
        );
        check_picture(&mut session);
        check_counter_advances(&mut session);
        check_state_size_rules(&mut session);
        check_slot_round_trip_and_guard(&mut session, &paths);
        let quality = runner_core_options::savestate_quality(&PS2);
        state_policy_core_tests::check_states_refused(&mut session, quality);
        check_reset_reboots(&mut session);
        run(&mut session, 20);
        SaveStateSlots::for_game(&paths)
            .save(&mut session, StateSlotId::Auto, None)
            .unwrap();
        counter(&mut session)
    };
    // The core flushes `.nvm` / `.mec` when its VM shuts down (first session dropped).
    check_core_files(&paths.system_dir, &paths.saves_dir, &stems);
    // Unload → load ×3 in one process, each resuming the auto state at startup.
    for round in 1..=3 {
        eprintln!("pcee2 suite: reload {round}: startup resume");
        let mut session = load();
        check_startup_resume(&mut session, &paths, auto_counter);
    }
}

/// The fixed 96 MiB state passes the PS2 cap (128 MiB), not the default (64 MiB).
fn check_state_size_rules(session: &mut CoreSession) {
    let size = session.current_state_size() as u64;
    eprintln!("pcee2 suite: state size {size}");
    assert!(size > DEFAULT_MAX_STATE_BYTES, "32 MB EE RAM + GS: {size}");
    let cap = runner_core_options::max_state_bytes(&PS2);
    assert!(check_state_size(SLOT, size, cap).is_ok());
    assert!(check_state_size(SLOT, size, DEFAULT_MAX_STATE_BYTES).is_err());
}

fn sidecar(paths: &RunnerDataPaths) -> StateSidecar {
    serde_json::from_slice(&std::fs::read(paths.states_dir.join("slot1.json")).unwrap()).unwrap()
}

fn refusal(slots: &SaveStateSlots, session: &mut CoreSession) -> String {
    format!("{:#}", slots.load(session, SLOT).expect_err("refused"))
}

/// Save, run on, load: the RAM counter is back at the saved value; then every
/// refusal of the runner's guard (damaged, other PCEE2 version, other disc).
fn check_slot_round_trip_and_guard(session: &mut CoreSession, paths: &RunnerDataPaths) {
    let slots = SaveStateSlots::for_game(paths);
    let started = Instant::now();
    slots.save(session, SLOT, None).unwrap();
    let saved_counter = counter(session);
    let saved = sidecar(paths);
    eprintln!(
        "pcee2 suite: {} bytes saved in {:?}",
        saved.state_len.unwrap(),
        started.elapsed()
    );
    let tag = saved
        .content_tag
        .clone()
        .expect("PS2 discs tag their states");
    assert!(tag.starts_with("NSTR_000.01;1:"), "{tag}");
    run(session, 30);
    assert!(counter(session) > saved_counter + 20);
    let started = Instant::now();
    slots.load(session, SLOT).unwrap();
    eprintln!("pcee2 suite: loaded in {:?}", started.elapsed());
    assert_eq!(counter(session), saved_counter, "RAM restored");
    run(session, 5);
    assert!(counter(session) > saved_counter, "runs on after the load");

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
        core_version: "v0.0.1".into(),
        ..saved.clone()
    };
    let text = refused_with(session, &version);
    assert_eq!(text, "state was made by another PCEE2 version");
    let other_disc = StateSidecar {
        content_tag: Some("NSTR_000.02;1:00000000".into()),
        ..saved.clone()
    };
    let text = refused_with(session, &other_disc);
    assert_eq!(text, "state was made from another disc or revision");
    std::fs::write(&json, serde_json::to_vec(&saved).unwrap()).unwrap();
    slots.load(session, SLOT).unwrap();
    assert_eq!(counter(session), saved_counter, "loads again");
}

/// `--load-state auto` as the runner does it: an attempt before the first frame
/// (no boot hold: the size is fixed, spike D4), retried after frames until answered;
/// resumes at the saved counter.
fn check_startup_resume(session: &mut CoreSession, paths: &RunnerDataPaths, auto_counter: u32) {
    let saves = GameSaveData::load(paths, session, 0);
    let quality = runner_core_options::savestate_quality(&PS2);
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
        run(session, 1);
        frames += 1;
    };
    eprintln!("pcee2 suite: resume answered after {frames} frames");
    assert!(outcome.loaded, "{}", outcome.status);
    assert_eq!(outcome.status, RESUMED);
    assert_eq!(counter(session), auto_counter, "RAM of the resume point");
    run(session, 10);
    assert!(counter(session) > auto_counter, "runs on after the resume");
}
