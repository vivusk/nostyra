//! Sega Dreamcast on Flycast (`scripts/fetch-flycast-core.*`) with the self-written
//! test program, through the offscreen OpenGL context (Flycast has no software
//! renderer, so the whole suite sits behind `gl_test_gate`). Flycast removes its
//! fault handler at `retro_deinit` (M11 phase-00 spike), so the suite shares the
//! runner's test binary.
//!
//! The ELF (HLE BIOS jumps straight to it): top half red (green while A is held,
//! yellow while the right trigger is pulled), bottom half blue with the frame counter
//! in green (white while the stick points right). States are `Serialized` but
//! frame-exact for this program (spike), so slots check an exact replay; run-ahead
//! and rewind are refused. The GDI pair behind `nostyra-test-dc.m3u` checks disc
//! swap (Flycast loads disc 2's image) and the per-game VMU file.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;
use std::sync::PoisonError;

use crate::dreamcast_real_core_checks_tests::{
    bottom, check_audio_flows, check_frames_and_input_pixels, run, top,
};
use libretro_host::CoreSession;
use system_catalog::DREAMCAST;

use crate::real_core_session_loader::{gl_test_gate, load_session_hw};
use crate::runner_held_input::HeldInput;
use crate::runner_real_core_suite::{core_path, repo, scratch, CORE_LOCK};
use crate::{runner_core_options, save_state_slots_core_tests, state_policy_core_tests};

const ELF: &str = "test-assets/roms/nostyra-test-dc.elf";
const M3U: &str = "test-assets/roms/nostyra-test-dc.m3u";
/// One VMU per pad, made by the core in the game's save folder (128 KiB).
const VMU: &str = "NSTY-00001.A1.bin";
const VMU_BYTES: u64 = 128 * 1024;

#[test]
fn dreamcast_suite_flycast() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let core = system_catalog::core_for_system(DREAMCAST.id).unwrap().id;
    if !core_path(core).exists() {
        eprintln!("skipped: run scripts/fetch-{core}-core first");
        return;
    }
    if !gl_test_gate() {
        return;
    }
    let provider = hw_render_gl::SdlGlProvider::new();
    let options = runner_core_options::resolve(&DREAMCAST, &[]);
    {
        eprintln!("{core} suite: test program (ELF)");
        let temp = scratch("core-suite-dc-elf");
        let mut session = load_session_hw(core, &repo(ELF), temp.path(), &options, &provider);
        assert!(session.uses_hw_context(), "Flycast renders through OpenGL");
        check_frames_and_input_pixels(&mut session);
        check_audio_flows(&mut session);
        save_state_slots_core_tests::check_state_slots(&mut session, temp.path(), true);
        check_reset_continues(&mut session);
        let quality = runner_core_options::savestate_quality(&DREAMCAST);
        state_policy_core_tests::check_states_refused(&mut session, quality);
    }
    eprintln!("{core} suite: two-disc GDI playlist");
    let temp = scratch("core-suite-dc-m3u");
    let mut session = load_session_hw(core, &repo(M3U), temp.path(), &options, &provider);
    check_disc_swap(&mut session);
    check_vmu_file(temp.path());
}

/// `retro_reset` reloads the program: frames continue and the pad still works (the
/// port devices survive a reset, spike).
fn check_reset_continues(session: &mut CoreSession) {
    session.reset();
    let video = run(session, HeldInput::None, 30);
    assert!(top(&video)[0] > 200, "picture back after reset");
    let pressed = top(&run(session, HeldInput::B, 3));
    assert!(
        pressed[1] > 200,
        "pad still connected after reset: {pressed:?}"
    );
}

/// Disc swap through the m3u's disk control: closing the tray on disc 2 makes Flycast
/// open and parse its GDI (`insertGdrom`; a broken image is refused), and the game
/// keeps running. A game reads the new disc itself (the test program does not);
/// `retro_reset` reboots the playlist's first disc (Flycast's `game_data`).
fn check_disc_swap(session: &mut CoreSession) {
    let first = bottom(&run(session, HeldInput::None, 20));
    assert!(
        first[0] < 50 && first[2] > 200,
        "disc 1: blue bottom {first:?}"
    );
    {
        let discs = session.discs().expect("disk control from the m3u");
        assert_eq!(discs.count(), 2);
        assert_eq!(discs.label(1), "nostyra-test-dc-disc2");
        discs.swap_to(1).unwrap();
        assert_eq!(discs.index(), 1);
    }
    let after = run(session, HeldInput::None, 10);
    assert!(top(&after)[0] > 200, "the game keeps running on disc 2");
    session.reset();
    let rebooted = bottom(&run(session, HeldInput::None, 30));
    assert!(
        rebooted[2] > 200,
        "reset boots the first disc again: {rebooted:?}"
    );
}

/// The core created the first pad's VMU in the game's save folder, pre-formatted.
fn check_vmu_file(save_dir: &Path) {
    let vmu = save_dir.join(VMU);
    let len = std::fs::metadata(&vmu).map(|m| m.len());
    assert_eq!(len.ok(), Some(VMU_BYTES), "{}", vmu.display());
}
