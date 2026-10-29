//! Nintendo DS on melonDS DS (`scripts/fetch-melondsds-core.*`) with the self-written
//! test ROMs (`test-assets/roms/src/build-nostyra-test-nds.py`): software renderer
//! (every OS), the play options with `threaded_renderer` as shipped (the one that is
//! picture-exact after a state load) and a fixed absolute start time (all six
//! `melonds_start_time_absolute_*` keys: without them the core aborts). Several
//! melonDS DS sessions and other cores share one process (M13 phase-00 spike), so
//! the suite lives in the runner's test binary; one system folder for the suite (the
//! core writes `melonDS DS/wfcsettings.bin` there), saves per session.
//!
//! ROM A (homebrew layout): picture, keys, touch, stick cursor, R3 layout, audio.
//! ROM B (retail-like layout, 8 KiB EEPROM boot counter): exact-size states, then
//! the generic run-ahead / slot / rewind / reset checks.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;
use std::sync::PoisonError;

use ipc_protocol::StateSlotId;
use libretro_host::{CoreSession, HostError};
use system_catalog::NDS;

use crate::real_core_session_loader::load_session_in;
use crate::run_ahead_core_tests::RunAheadExpectations;
use crate::runner_core_options;
use crate::runner_held_input::HeldInput;
use crate::runner_real_core_suite::{core_path, repo, run_core_checks, scratch, CORE_LOCK};
use crate::state_load_checks::check_state_size;
use checks::{
    check_audio_flows, check_frames_and_keys, check_layout_swap, check_stick_cursor_touch,
    check_top_screen_press_is_no_touch, check_touch_quadrants,
};

#[path = "nds_real_core_checks_tests.rs"]
mod checks;

const ROM_A: &str = "test-assets/roms/nostyra-test.nds";
const ROM_B: &str = "test-assets/roms/nostyra-test-save.nds";
/// ROM B's cartridge save: 64 kbit EEPROM (game code not in melonDS's ROM list).
const EEPROM_BYTES: usize = 8 * 1024;

/// Spike: with the threaded renderer the frames after a state load are exact, the
/// state itself (~530 bytes) and the audio stream are not; restoring twice gives
/// identical states.
const MELONDS_DS_RUN_AHEAD: RunAheadExpectations = RunAheadExpectations {
    exact_state: false,
    exact_audio: false,
    exact_round_trip: true,
    state_jitter_bytes: 0,
};

/// Play options + a fixed start time 2004-01-01 00:00:00.
fn test_options() -> Vec<(String, String)> {
    let absolute = [
        ("mode", "absolute"),
        ("absolute_year", "2004"),
        ("absolute_month", "1"),
        ("absolute_day", "1"),
        ("absolute_hour", "0"),
        ("absolute_minute", "0"),
        ("absolute_second", "0"),
    ]
    .map(|(key, value)| (format!("melonds_start_time_{key}"), value.to_owned()));
    let options = runner_core_options::resolve(&NDS, &absolute);
    let threaded = ("melonds_threaded_renderer".to_owned(), "enabled".to_owned());
    assert!(options.contains(&threaded), "tests use the play value");
    options
}

#[test]
fn nds_suite_melonds_ds() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let core = system_catalog::core_for_system(NDS.id).unwrap().id;
    if !core_path(core).exists() {
        eprintln!("skipped: run scripts/fetch-melondsds-core first");
        return;
    }
    let system = scratch("core-suite-nds-system");
    let options = test_options();
    let load = |rom: &str, saves: &Path| {
        load_session_in(core, &repo(rom), (system.path(), saves), &options, None)
    };
    {
        eprintln!("melonds_ds suite: ROM A");
        let saves = scratch("core-suite-nds-a");
        let mut session = load(ROM_A, saves.path());
        assert!(!session.uses_hw_context(), "software renderer");
        check_frames_and_keys(&mut session);
        check_touch_quadrants(&mut session);
        check_top_screen_press_is_no_touch(&mut session);
        check_stick_cursor_touch(&mut session);
        check_layout_swap(&mut session);
        check_audio_flows(&mut session);
    }
    eprintln!("melonds_ds suite: ROM B");
    let saves = scratch("core-suite-nds-b");
    let mut session = load(ROM_B, saves.path());
    check_eeprom_boot_counter(&mut session);
    check_exact_size_states(&mut session);
    let states = saves.path().join("states");
    std::fs::create_dir_all(&states).unwrap();
    run_core_checks(&mut session, &states, MELONDS_DS_RUN_AHEAD);
    check_reset_keeps_state_size(&mut session);
}

/// The ARM7 wrote `[boot counter 1, 0xA5]` to the blank 8 KiB EEPROM over AUXSPI.
fn check_eeprom_boot_counter(session: &mut CoreSession) {
    assert_eq!(
        session.save_ram_mut().map(|ram| ram.len()),
        Some(EEPROM_BYTES)
    );
    checks_run(session, 10);
    let ram = session.save_ram_mut().unwrap();
    assert_eq!(ram[..2], [1, 0xA5], "first boot on a blank EEPROM");
    assert!(ram[2..].iter().all(|&b| b == 0), "rest untouched");
}

/// ~18.3 MiB state (under the 128 MiB melonDS DS cap) round-trips; one byte short or
/// long is refused by the host (exact state size) before the core sees it.
fn check_exact_size_states(session: &mut CoreSession) {
    let size = session.current_state_size();
    assert!((18 << 20..20 << 20).contains(&size), "state size {size}");
    let cap = runner_core_options::max_state_bytes(&NDS);
    assert!(check_state_size(StateSlotId::Numbered(1), size as u64, cap).is_ok());
    let state = session.serialize().unwrap();
    assert_eq!(state.len(), size);
    for wrong in [&state[..size - 1], &[state.as_slice(), &[0]].concat()] {
        let refused = session.unserialize(wrong);
        assert!(matches!(refused, Err(HostError::UnserializeFailed)));
    }
    session.unserialize(&state).unwrap();
}

/// `retro_reset` keeps the state size (no cache clear needed in the runner).
fn check_reset_keeps_state_size(session: &mut CoreSession) {
    let before = session.current_state_size();
    session.reset();
    checks_run(session, 5);
    assert_eq!(session.current_state_size(), before);
}

fn checks_run(session: &mut CoreSession, frames: usize) {
    crate::dreamcast_real_core_checks_tests::run(session, HeldInput::None, frames);
}
