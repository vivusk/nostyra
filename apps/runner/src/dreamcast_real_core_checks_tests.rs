//! Frame, input and audio checks for the Dreamcast test program (see
//! `dreamcast_real_core_suite_tests`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use libretro_host::{AudioSink, CoreSession};
use video_renderer::RgbaFrameBuffer;

use crate::runner_held_input::HeldInput;

/// Runs `frames` frames holding `held`; returns the last picture.
pub(crate) fn run(session: &mut CoreSession, held: HeldInput, frames: usize) -> RgbaFrameBuffer {
    let mut video = RgbaFrameBuffer::default();
    let (mut audio, mut input) = (AudioCount::default(), held);
    for _ in 0..frames {
        session
            .run_frame(&mut video, &mut audio, &mut input)
            .unwrap();
    }
    video
}

/// RGB at (`fx`, `fy`) as fractions of the picture size.
pub(crate) fn pixel(video: &RgbaFrameBuffer, fx: f32, fy: f32) -> [u8; 3] {
    let (width, height) = video.size();
    let (x, y) = ((width as f32 * fx) as u32, (height as f32 * fy) as u32);
    let at = ((y * width + x) * 4) as usize;
    video.pixels()[at..at + 3].try_into().unwrap()
}

pub(crate) fn top(video: &RgbaFrameBuffer) -> [u8; 3] {
    pixel(video, 0.5, 0.05) // near the top row: an upside-down picture shows blue here
}

pub(crate) fn bottom(video: &RgbaFrameBuffer) -> [u8; 3] {
    pixel(video, 0.5, 0.75)
}

/// Upright red/blue picture whose bottom changes every frame; A (RetroPad B) turns the
/// top green, the right analog trigger yellow, the left stick right the bottom white.
pub(crate) fn check_frames_and_input_pixels(session: &mut CoreSession) {
    let video = run(session, HeldInput::None, 10); // past boot
    assert_eq!(video.size(), (640, 480));
    let [r, g, b] = top(&video);
    assert!(
        r > 200 && g < 50 && b < 50,
        "top red (upright): {:?}",
        [r, g, b]
    );
    let blue = bottom(&video);
    assert!(blue[0] < 50 && blue[2] > 200, "bottom blue: {blue:?}");
    let greens: std::collections::BTreeSet<u8> = (0..8)
        .map(|_| bottom(&run(session, HeldInput::None, 1))[1])
        .collect();
    assert!(
        greens.len() > 1,
        "frame counter changes the bottom: {greens:?}"
    );
    let pressed = top(&run(session, HeldInput::B, 3));
    assert!(
        pressed[1] > 200 && pressed[0] < 50,
        "A held: top green {pressed:?}"
    );
    let trigger = top(&run(session, HeldInput::RightTrigger, 3));
    assert!(
        trigger.iter().take(2).all(|&c| c > 200),
        "RT: top yellow {trigger:?}"
    );
    let stick = bottom(&run(session, HeldInput::LeftStickRight, 3));
    assert!(
        stick.iter().all(|&c| c > 200),
        "stick right: bottom white {stick:?}"
    );
    let released = run(session, HeldInput::None, 3);
    assert!(
        top(&released)[0] > 200 && top(&released)[1] < 50,
        "released: red again"
    );
}

/// Stereo frames delivered (Flycast pushes 44.1 kHz audio, silence here).
#[derive(Default)]
struct AudioCount(usize);

impl AudioSink for AudioCount {
    fn samples(&mut self, interleaved: &[i16]) {
        self.0 += interleaved.len() / 2;
    }
}

/// ~735 stereo frames per video frame (44 100 Hz / 59.94).
pub(crate) fn check_audio_flows(session: &mut CoreSession) {
    let mut audio = AudioCount::default();
    let (mut video, mut input) = (RgbaFrameBuffer::default(), HeldInput::None);
    for _ in 0..60 {
        session
            .run_frame(&mut video, &mut audio, &mut input)
            .unwrap();
    }
    let per_frame = audio.0 / 60;
    assert!(
        (700..=770).contains(&per_frame),
        "{per_frame} stereo frames per frame"
    );
}
