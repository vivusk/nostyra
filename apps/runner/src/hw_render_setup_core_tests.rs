//! `HwRenderSetup` with real cores: software cores never touch SDL video; an OpenGL
//! core (Mupen64Plus-Next) gets its context and upright frames. The N64 check skips
//! (with a note) without the core, the N64 test ROM or OpenGL.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use libretro_host::{NullAudioSink, NullInputSource};
use video_renderer::RgbaFrameBuffer;

use crate::hw_render_setup::HwRenderSetup;
use crate::real_core_session_loader::{core_path, gl_test_gate, load_session_hw};
use crate::runner_core_options;
use crate::runner_real_core_suite::{repo, scratch, CORE_LOCK};

#[test]
fn software_core_never_initialises_sdl_video() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let setup = HwRenderSetup::new();
    let rom = repo("test-assets/roms/nostyra-test.sfc");
    let temp = scratch("hw-setup-snes");
    let mut session = load_session_hw("snes9x", &rom, temp.path(), &[], setup.as_provider());
    let (mut video, mut audio) = (RgbaFrameBuffer::default(), NullAudioSink::default());
    for _ in 0..5 {
        session
            .run_frame(&mut video, &mut audio, &mut NullInputSource)
            .unwrap();
    }
    assert!(!session.uses_hw_context());
    drop(session);
    assert!(!setup.video_initialised(), "SDL video started for snes9x");
}

#[test]
fn opengl_core_renders_upright_frames_through_the_setup() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let system = &system_catalog::NINTENDO_64;
    let core_id = system_catalog::core_for_system(system.id).unwrap().id;
    let rom = repo("test-assets/roms/nostyra-test.z64");
    if !core_path(core_id).exists() || !rom.exists() {
        eprintln!("skipped: needs the {core_id} core and {}", rom.display());
        return;
    }
    if !gl_test_gate() {
        return;
    }
    let setup = HwRenderSetup::new();
    let temp = scratch("hw-setup-n64");
    let options = runner_core_options::resolve(system, &[]);
    let mut session = load_session_hw(core_id, &rom, temp.path(), &options, setup.as_provider());
    assert!(session.uses_hw_context());
    assert!(setup.video_initialised());
    let (mut video, mut audio) = (RgbaFrameBuffer::default(), NullAudioSink::default());
    for _ in 0..120 {
        session
            .run_frame(&mut video, &mut audio, &mut NullInputSource)
            .unwrap();
    }
    let (width, height) = video.size();
    let pixel = |y: u32| {
        let at = ((y * width + width / 2) * 4) as usize;
        video.pixels()[at..at + 3].to_vec()
    };
    // The test ROM fills the top half red and the bottom half blue.
    let (top, bottom) = (pixel(height / 4), pixel(height * 3 / 4));
    assert!(top[0] > 200 && top[2] < 50, "top {top:?}");
    assert!(bottom[2] > 200 && bottom[0] < 50, "bottom {bottom:?}");
}
