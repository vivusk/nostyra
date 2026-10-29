//! Running frames and reading the test program's MEM1 words (boot, pad words, audio,
//! reset); shared by the Null and OpenGL Dolphin suites.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::time::{Duration, Instant};

use libretro_host::{AudioSink, CoreSession};
use video_renderer::RgbaFrameBuffer;

use super::{debug_word, FIELDS, MAGIC, PI_CAUSE, RESET_SEEN, SI_IN_HI, SI_IN_LO};
use crate::runner_held_input::HeldInput;

pub(crate) const GC_DOL: &str = "test-assets/roms/nostyra-test.dol";
pub(crate) const WII_DOL: &str = "test-assets/roms/nostyra-test-wii.dol";

/// Longest a debug-build Dolphin may take to reach the program's main loop.
const BOOT_LIMIT: Duration = Duration::from_secs(30);

/// Peak absolute sample and stereo frames delivered.
#[derive(Default)]
pub(crate) struct Audio {
    pub(crate) peak: i32,
    pub(crate) frames: usize,
}

impl AudioSink for Audio {
    fn samples(&mut self, interleaved: &[i16]) {
        let peak = interleaved.iter().map(|&s| i32::from(s).abs()).max();
        self.peak = self.peak.max(peak.unwrap_or(0));
        self.frames += interleaved.len() / 2;
    }
}

/// Picture sink that also counts new frames (Dolphin presents only changed XFBs and
/// dupes the rest).
#[derive(Default)]
pub(crate) struct Frames {
    pub(crate) last: RgbaFrameBuffer,
    pub(crate) presented: usize,
}

impl libretro_host::VideoSink for Frames {
    fn frame(&mut self, frame: &libretro_host::VideoFrame<'_>) {
        self.presented += 1;
        self.last.frame(frame);
    }

    fn dupe(&mut self, width: u32, height: u32) {
        self.last.dupe(width, height);
    }
}

/// Runs `frames` frames holding `held`; returns the picture sink and the audio seen.
pub(crate) fn run(session: &mut CoreSession, held: HeldInput, frames: usize) -> (Frames, Audio) {
    let (mut video, mut audio, mut input) = (Frames::default(), Audio::default(), held);
    for _ in 0..frames {
        session
            .run_frame(&mut video, &mut audio, &mut input)
            .unwrap();
    }
    (video, audio)
}

/// Runs frames until the program's MEM1 block carries its magic and counts fields
/// (main loop reached); returns the frames it took.
pub(crate) fn boot(session: &mut CoreSession) -> usize {
    let started = Instant::now();
    let mut frames = 0;
    while debug_word(session, 0) != MAGIC || debug_word(session, FIELDS) < 4 {
        assert!(
            started.elapsed() < BOOT_LIMIT,
            "DOL main loop within {BOOT_LIMIT:?}"
        );
        run(session, HeldInput::None, 1);
        frames += 1;
    }
    frames
}

/// The SI channel 0 words (IN_HI, IN_LO) after a few frames holding `held`.
pub(crate) fn pad_words(session: &mut CoreSession, held: HeldInput) -> (u32, u32) {
    run(session, held, 4);
    (debug_word(session, SI_IN_HI), debug_word(session, SI_IN_LO))
}

/// The field counter advances by one per `retro_run` (Dolphin runs one VI field).
pub(crate) fn check_field_counter(session: &mut CoreSession) {
    let before = debug_word(session, FIELDS);
    run(session, HeldInput::None, 30);
    let advanced = debug_word(session, FIELDS) - before;
    assert!(
        (25..=35).contains(&advanced),
        "fields in 30 frames: {advanced}"
    );
}

/// The AI-DMA square wave (+-0x2000) reaches the host at 48 kHz.
pub(crate) fn check_audio(session: &mut CoreSession) {
    let (_, audio) = run(session, HeldInput::None, 30);
    assert!(
        audio.peak > 4000,
        "square wave audible (peak {})",
        audio.peak
    );
    assert!(
        audio.frames > 30 * 700,
        "{} stereo frames in 30 frames",
        audio.frames
    );
}

/// `retro_reset` taps the console's reset button (Dolphin: PI reset switch, held ~30 fields),
/// which the program latches; a game's own handler then reboots (the DOL has none).
pub(crate) fn check_reset(session: &mut CoreSession) {
    assert_eq!(debug_word(session, RESET_SEEN), 0, "not pressed yet");
    let up = debug_word(session, PI_CAUSE);
    assert_ne!(up & 1 << 16, 0, "reset button up: {up:08X}");
    session.reset();
    run(session, HeldInput::None, 10);
    assert_eq!(debug_word(session, RESET_SEEN), 1, "reset button pressed");
    run(session, HeldInput::None, 50); // held ~30 fields here
    let after = debug_word(session, PI_CAUSE);
    assert_ne!(after & 1 << 16, 0, "released again: {after:08X}");
}
