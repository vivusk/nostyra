//! Picture, key, touch, stick-cursor, layout and audio checks for the DS test ROM A
//! (submodule of `nds_real_core_suite_tests`). Colours are read back as the DS's
//! 5-bit channels: top = red (green while a key is held), bottom = blue while the pen
//! is up, (x / 8, y / 8, 0) while it is down; the frame counter's low 3 bits sit in
//! the top's blue and the bottom's red.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::collections::BTreeSet;

use input_mapping::ANALOG_MAX;
use libretro_host::{AudioSink, CoreSession};
use system_catalog::{touch_rect, TouchScreenRule};
use video_renderer::{RgbaFrameBuffer, ViewportRect};

use crate::dreamcast_real_core_checks_tests::{pixel, run};
use crate::emulation_main_loop::pointer_events::cursor_to_pointer;
use crate::runner_held_input::HeldInput;

/// RetroPad R2 (touch at the stick cursor) and R3 (swap the screen layout).
const R2: u32 = 13;
const R3: u32 = 15;

/// 8-bit channel → the DS's 5-bit value.
fn c5(channel: u8) -> u8 {
    ((u32::from(channel) * 31 + 127) / 255) as u8
}

fn ds_colour(rgb: [u8; 3]) -> [u8; 3] {
    rgb.map(c5)
}

/// Top screen, right of the 3D triangle (top-left) — top-bottom layout.
fn top(video: &RgbaFrameBuffer) -> [u8; 3] {
    ds_colour(pixel(video, 0.9, 0.25))
}

/// Bottom screen, bottom-left corner: away from every touch point (the core draws
/// its cursor under a touch).
fn bottom(video: &RgbaFrameBuffer) -> [u8; 3] {
    ds_colour(pixel(video, 4.0 / 256.0, (192.0 + 188.0) / 384.0))
}

fn pen_up(colour: [u8; 3]) -> bool {
    colour[1] == 0 && colour[2] == 31
}

/// Upright 256×384 picture (top red, bottom blue) that changes every frame; a held
/// key turns the top green, released → red again.
pub(crate) fn check_frames_and_keys(session: &mut CoreSession) {
    let video = run(session, HeldInput::None, 30);
    assert_eq!(video.size(), (256, 384), "software renderer, top-bottom");
    let [r, g, _] = top(&video);
    assert_eq!((r, g), (31, 0), "top red (upright): {:?}", top(&video));
    assert!(pen_up(bottom(&video)), "bottom blue: {:?}", bottom(&video));
    let frames: BTreeSet<u8> = (0..8)
        .map(|_| top(&run(session, HeldInput::None, 1))[2])
        .collect();
    assert!(
        frames.len() > 1,
        "frame counter changes the top: {frames:?}"
    );
    let held = top(&run(session, HeldInput::B, 3));
    assert_eq!((held[0], held[1]), (0, 31), "key held: top green {held:?}");
    let released = top(&run(session, HeldInput::None, 3));
    assert_eq!((released[0], released[1]), (31, 0), "released: red again");
}

/// A touch in each bottom-screen quadrant tints the bottom with its position
/// (x / 8, y / 8, ±1 for the controller's 7-bit reading); release → blue again.
pub(crate) fn check_touch_quadrants(session: &mut CoreSession) {
    for (x, y) in [(64, 48), (192, 48), (64, 144), (192, 144)] {
        let tint = bottom(&run(session, HeldInput::Touch { x, y }, 4));
        let (want_r, want_g) = ((x / 8) as u8, (y / 8) as u8);
        assert!(
            tint[0].abs_diff(want_r) <= 1 && tint[1].abs_diff(want_g) <= 1 && tint[2] == 0,
            "touch ({x}, {y}): tint {tint:?}, want ({want_r}, {want_g}, 0)"
        );
        let up = bottom(&run(session, HeldInput::None, 4));
        assert!(pen_up(up), "released after ({x}, {y}): {up:?}");
    }
}

/// A mouse press on the top screen goes through the runner's mapping
/// (`cursor_to_pointer` + the DS touch rect): not pressed, so the core sees no touch
/// (it would clamp a press to the bottom screen's top edge). On the bottom screen
/// the same mapping does press.
pub(crate) fn check_top_screen_press_is_no_touch(session: &mut CoreSession) {
    let viewport = ViewportRect {
        x: 0.0,
        y: 0.0,
        width: 256.0,
        height: 384.0,
    };
    let touch = touch_rect(TouchScreenRule::DsBottomScreen, 256, 384);
    let press = |cursor| cursor_to_pointer(Some(viewport), Some(cursor), true, touch);
    let top_press = press((128.0, 96.0));
    assert!(!top_press.pressed && !top_press.offscreen);
    let seen = bottom(&run(session, HeldInput::Pointer(top_press), 4));
    assert!(pen_up(seen), "top-screen press: no touch {seen:?}");
    let bottom_press = press((128.0, 288.0));
    let seen = bottom(&run(session, HeldInput::Pointer(bottom_press), 4));
    assert!(!pen_up(seen), "bottom-screen press touches {seen:?}");
    run(session, HeldInput::None, 4);
}

/// The right stick moves the core's cursor without touching; R2 touches at it
/// (`touch_mode=auto`); R2 released → pen up.
pub(crate) fn check_stick_cursor_touch(session: &mut CoreSession) {
    let moved = bottom(&run(
        session,
        HeldInput::RightStick(ANALOG_MAX, ANALOG_MAX),
        20,
    ));
    assert!(pen_up(moved), "stick alone does not touch: {moved:?}");
    let touched = bottom(&run(session, HeldInput::Joypad(R2), 4));
    assert!(
        !pen_up(touched) && touched[2] == 0,
        "R2 touches: {touched:?}"
    );
    let up = bottom(&run(session, HeldInput::None, 4));
    assert!(pen_up(up), "R2 released: {up:?}");
}

/// R3 swaps to the second layout (left-right, 512×192), R3 again back (256×384);
/// the core reports each change through `SET_GEOMETRY`.
pub(crate) fn check_layout_swap(session: &mut CoreSession) {
    session.take_av_change(); // the core repeats 256×384 once at start
    for want in [(512, 192), (256, 384)] {
        run(session, HeldInput::Joypad(R3), 1);
        let video = run(session, HeldInput::None, 3);
        let changed = session.take_av_change().expect("geometry change reported");
        let geometry = (changed.geometry.base_width, changed.geometry.base_height);
        assert_eq!(geometry, want, "R3: layout geometry");
        assert_eq!(video.size(), want, "R3: frame size");
    }
}

/// Stereo frames delivered and the loudest sample.
#[derive(Default)]
struct AudioStats {
    frames: usize,
    peak: u16,
}

impl AudioSink for AudioStats {
    fn samples(&mut self, interleaved: &[i16]) {
        self.frames += interleaved.len() / 2;
        let peak = interleaved.iter().map(|s| s.unsigned_abs()).max();
        self.peak = self.peak.max(peak.unwrap_or(0));
    }
}

/// ~547 stereo frames per video frame (32 728.5 Hz / 59.83), the PSG tone audible.
pub(crate) fn check_audio_flows(session: &mut CoreSession) {
    let mut audio = AudioStats::default();
    let (mut video, mut input) = (RgbaFrameBuffer::default(), HeldInput::None);
    for _ in 0..60 {
        session
            .run_frame(&mut video, &mut audio, &mut input)
            .unwrap();
    }
    let per_frame = audio.frames / 60;
    assert!(
        (520..=575).contains(&per_frame),
        "{per_frame} stereo frames per frame"
    );
    assert!(audio.peak > 1000, "tone audible (peak {})", audio.peak);
}
