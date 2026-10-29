//! Nintendo 64 on Mupen64Plus-Next (`scripts/fetch-mupen64plus-next-core.*`) with the
//! self-written `nostyra-test.z64`, twice in one session-at-a-time suite: the
//! Angrylion software renderer (no OpenGL needed: runs everywhere) and GLideN64 through
//! the offscreen OpenGL context (behind `gl_test_gate`). Both renderers load and unload
//! in one process (M10 phase-00 spike: Angrylion threads are joined at deinit).
//!
//! Mupen's states are `Serialized`: serialize runs the core to a safe point, so replay
//! after a load is not checksum-exact (slots check with `exact_replay = false`), and
//! run-ahead / rewind are refused (`check_states_refused`; `run_state_checks` is not
//! used). Frames and input are checked on pixels: the ROM fills the top half red
//! (green while A is held) and the bottom half blue with the frame counter in green
//! (white while the stick points right).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use libretro_host::{CoreSession, HostError};
use system_catalog::NINTENDO_64;
use video_renderer::RgbaFrameBuffer;

use crate::real_core_session_loader::{gl_test_gate, load_session_hw};
use crate::runner_held_input::HeldInput;
use crate::runner_real_core_suite::{
    core_path, load_session_with, repo, scratch, LoudestSample, CORE_LOCK,
};
use crate::{reset_core_tests, runner_core_options, save_state_slots_core_tests};
use crate::{sram_persistence, state_policy_core_tests};

const ROM: &str = "test-assets/roms/nostyra-test.z64";
/// Mupen64Plus-Next's SAVE_RAM: EEPROM, 4 memory paks, SRAM and Flash in one blob.
const SAVE_RAM_BYTES: usize = 296_960;

#[test]
fn runner_real_core_suite_mupen64plus_next() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let core = system_catalog::core_for_system(NINTENDO_64.id).unwrap().id;
    if !core_path(core).exists() {
        eprintln!("skipped: run scripts/fetch-{core}-core first");
        return;
    }
    {
        eprintln!("{core} suite: angrylion");
        let temp = scratch("core-suite-n64-angrylion");
        let angrylion = [("mupen64plus-rdp-plugin".to_owned(), "angrylion".to_owned())];
        let options = runner_core_options::resolve(&NINTENDO_64, &angrylion);
        let mut session = load_session_with(core, &repo(ROM), temp.path(), &options);
        assert!(!session.uses_hw_context(), "Angrylion draws in software");
        assert_eq!(
            session.save_ram_mut().map(|ram| ram.len()),
            Some(SAVE_RAM_BYTES)
        );
        check_frames_and_input_pixels(&mut session);
        check_eeprom_boot_counter(&mut session);
        save_state_slots_core_tests::check_state_slots(&mut session, temp.path(), false);
        check_wrong_size_state_refused(&mut session);
        reset_core_tests::check_reset(&mut session);
        let quality = runner_core_options::savestate_quality(&NINTENDO_64);
        state_policy_core_tests::check_states_refused(&mut session, quality);
    }
    if !gl_test_gate() {
        return;
    }
    eprintln!("{core} suite: gliden64");
    let provider = hw_render_gl::SdlGlProvider::new();
    let temp = scratch("core-suite-n64-gl");
    let options = runner_core_options::resolve(&NINTENDO_64, &[]);
    let mut session = load_session_hw(core, &repo(ROM), temp.path(), &options, &provider);
    assert!(session.uses_hw_context(), "GLideN64 renders through OpenGL");
    check_frames_and_input_pixels(&mut session);
    save_state_slots_core_tests::check_state_slots(&mut session, temp.path(), false);
}

/// Runs `frames` frames holding `held`; returns the last picture and the audio peak.
fn run(session: &mut CoreSession, held: HeldInput, frames: usize) -> (RgbaFrameBuffer, i32) {
    let (mut video, mut audio) = (RgbaFrameBuffer::default(), LoudestSample(0));
    let mut input = held;
    for _ in 0..frames {
        session
            .run_frame(&mut video, &mut audio, &mut input)
            .unwrap();
    }
    (video, audio.0)
}

/// RGB at (`fx`, `fy`) as fractions of the picture size.
fn pixel(video: &RgbaFrameBuffer, fx: f32, fy: f32) -> [u8; 3] {
    let (width, height) = video.size();
    let (x, y) = ((width as f32 * fx) as u32, (height as f32 * fy) as u32);
    let at = ((y * width + x) * 4) as usize;
    video.pixels()[at..at + 3].try_into().unwrap()
}

fn top(video: &RgbaFrameBuffer) -> [u8; 3] {
    pixel(video, 0.5, 0.05) // near the top row: an upside-down picture shows blue here
}

fn bottom(video: &RgbaFrameBuffer) -> [u8; 3] {
    pixel(video, 0.5, 0.75)
}

/// Upright red/blue picture whose bottom changes every frame; A (RetroPad B) turns the
/// top green, the left stick right turns the bottom white; the tone is audible.
fn check_frames_and_input_pixels(session: &mut CoreSession) {
    let (video, peak) = run(session, HeldInput::None, 10); // past boot
    assert!(peak > 1000, "square wave is audible (peak {peak})");
    let [r, g, b] = top(&video);
    assert!(
        r > 200 && g < 50 && b < 50,
        "top red (upright): {:?}",
        [r, g, b]
    );
    let blue = bottom(&video);
    assert!(blue[0] < 50 && blue[2] > 200, "bottom blue: {blue:?}");
    let greens: std::collections::BTreeSet<u8> = (0..8)
        .map(|_| bottom(&run(session, HeldInput::None, 1).0)[1])
        .collect();
    assert!(
        greens.len() > 1,
        "frame counter changes the bottom: {greens:?}"
    );
    let pressed = top(&run(session, HeldInput::B, 5).0);
    assert!(
        pressed[1] > 200 && pressed[0] < 50,
        "A held: top green {pressed:?}"
    );
    let stick = bottom(&run(session, HeldInput::LeftStickRight, 5).0);
    assert!(
        stick.iter().all(|&c| c > 200),
        "stick right: bottom white {stick:?}"
    );
    let released = run(session, HeldInput::None, 5).0;
    assert!(top(&released)[0] > 200, "A released: top red again");
}

/// The ROM's boot code wrote EEPROM block 0 = `[boot counter, 0xA5]` at SAVE_RAM 0.
fn check_eeprom_boot_counter(session: &mut CoreSession) {
    let ram = session.save_ram_mut().unwrap();
    assert_eq!(ram[..2], [1, 0xA5], "first boot on a blank EEPROM");
    assert!(sram_persistence::sram_crc32(session).is_some());
}

/// A truncated or padded state is refused by the host (exact state size) instead of
/// reaching `retro_unserialize`, which would never return on it; a whole one loads.
fn check_wrong_size_state_refused(session: &mut CoreSession) {
    let state = session.serialize().unwrap();
    for wrong in [
        &state[..state.len() / 2],
        &[state.as_slice(), &[0]].concat(),
    ] {
        let refused = session.unserialize(wrong);
        assert!(matches!(refused, Err(HostError::UnserializeFailed)));
    }
    session.unserialize(&state).unwrap();
}
