//! Boot wait, frame, input, audio and memory-stick checks for the PSP test program
//! (submodule of `psp_real_core_suite_tests`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;
use std::time::{Duration, Instant};

use libretro_host::{AudioSink, CoreSession};
use video_renderer::RgbaFrameBuffer;

use crate::dreamcast_real_core_checks_tests::{bottom, top};
use crate::runner_held_input::HeldInput;
use crate::startup_boot_gate::{BootGate, BOOT_WAIT};

/// PPSSPP boots on its own thread: frames are paced a little so the boot can finish.
const BOOT_PACE: Duration = Duration::from_millis(2);

/// Stereo frames delivered.
#[derive(Default)]
pub(crate) struct AudioCount(pub usize);

impl AudioSink for AudioCount {
    fn samples(&mut self, interleaved: &[i16]) {
        self.0 += interleaved.len() / 2;
    }
}

/// Runs `frames` frames holding `held`; returns the last picture and the audio count.
pub(crate) fn run(
    session: &mut CoreSession,
    held: HeldInput,
    frames: usize,
) -> (RgbaFrameBuffer, usize) {
    let mut video = RgbaFrameBuffer::default();
    let (mut audio, mut input) = (AudioCount::default(), held);
    for _ in 0..frames {
        session
            .run_frame(&mut video, &mut audio, &mut input)
            .unwrap();
    }
    (video, audio.0)
}

/// Runs paced frames until the state size leaves its pre-boot value (the program is
/// loaded), then `settle` more; panics after [`BOOT_WAIT`].
pub(crate) fn boot(session: &mut CoreSession, settle: usize) {
    let (mut gate, start) = (BootGate::default(), Instant::now());
    while !gate.passed(session.current_state_size(), Instant::now()) {
        run(session, HeldInput::None, 1);
        std::thread::sleep(BOOT_PACE);
    }
    assert!(start.elapsed() < BOOT_WAIT, "booted within {BOOT_WAIT:?}");
    run(session, HeldInput::None, settle);
}

/// Upright red/blue 480×270 picture whose bottom changes every frame; ✕ (RetroPad B)
/// turns the top green, the stick right the bottom yellow; the tone flows.
pub(crate) fn check_frames_input_and_audio(session: &mut CoreSession) {
    let (video, _) = run(session, HeldInput::None, 2);
    assert_eq!(video.size(), (480, 270), "software backend, 16:9 crop");
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
    let pressed = top(&run(session, HeldInput::B, 4).0);
    assert!(
        pressed[1] > 200 && pressed[0] < 50,
        "✕ held: top green {pressed:?}"
    );
    let stick = bottom(&run(session, HeldInput::LeftStickRight, 4).0);
    assert!(
        stick[0] > 200 && stick[1] > 200,
        "stick right: bottom yellow {stick:?}"
    );
    let released = top(&run(session, HeldInput::None, 4).0);
    assert!(released[0] > 200 && released[1] < 50, "released: red again");
    let (_, samples) = run(session, HeldInput::None, 60);
    let per_frame = samples / 60;
    assert!(
        (600..=900).contains(&per_frame),
        "{per_frame} stereo frames per frame"
    );
}

/// The boot-time AUTOSAVE landed on the memory stick = the game's save folder
/// (`DATA.BIN`: the 16 data bytes as PPSSPP stores them, 32 bytes; spike).
pub(crate) fn check_memory_stick_save(save_dir: &Path) {
    let save = save_dir.join("PSP/SAVEDATA/NSTY00001DATA00");
    let data = std::fs::metadata(save.join("DATA.BIN")).map(|m| m.len());
    assert_eq!(data.ok(), Some(32), "{}", save.display());
    assert!(save.join("PARAM.SFO").is_file());
}
