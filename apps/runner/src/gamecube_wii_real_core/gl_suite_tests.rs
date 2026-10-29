//! Dolphin's OpenGL renderer (behind `gl_test_gate`): the test DOLs' XFB picture
//! reaches the host through our FBO. Non-black colour bars, a frame block that changes
//! every field (Dolphin presents only changed XFBs: dupes are tolerated, never
//! required), a pad block that turns green while A is held; base geometry 640×528
//! (EFB scale 1). Readback cost is logged (run_frame incl. the GL readback).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::collections::BTreeSet;
use std::sync::PoisonError;
use std::time::{Duration, Instant};

use libretro_host::CoreSession;
use system_catalog::{SystemSpec, GC, WII};
use video_renderer::RgbaFrameBuffer;

use super::checks::{boot, run};
use super::{dolphin, load_dol, options, system_folder, GC_DOL, WII_DOL};
use crate::dreamcast_real_core_checks_tests::pixel;
use crate::real_core_session_loader::gl_test_gate;
use crate::runner_held_input::HeldInput;
use crate::runner_real_core_suite::{scratch, CORE_LOCK};

/// Block centres as fractions of the 640×480 XFB: frame block (64, 64), pad block
/// (160, 64), first colour bar (40, 400).
const FRAME_BLOCK: (f32, f32) = (0.1, 0.133);
const PAD_BLOCK: (f32, f32) = (0.25, 0.133);
const WHITE_BAR: (f32, f32) = (0.0625, 0.8);

#[test]
fn gamecube_wii_gl_suite_dolphin() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(core) = dolphin() else { return };
    if !gl_test_gate() {
        return;
    }
    let system = system_folder(core);
    let provider = hw_render_gl::SdlGlProvider::new();
    for (spec, rom) in [(&GC, GC_DOL), (&WII, WII_DOL)] {
        eprintln!("dolphin gl suite: {rom}");
        let saves = scratch("core-suite-dolphin-gl");
        let dirs = (system.path(), saves.path());
        let mut session = load_dol(core, rom, dirs, &options(spec, &[]), Some(&provider));
        assert!(session.uses_hw_context(), "OpenGL Core 3.3 context");
        let geometry = session.av_info().geometry;
        let base = (geometry.base_width, geometry.base_height);
        assert_eq!(base, (640, 528), "EFB scale 1");
        boot(&mut session);
        check_picture_changes_per_field(&mut session);
        if spec.id == GC.id {
            check_pad_block(&mut session);
        }
        log_frame_cost(&mut session, spec);
    }
}

fn is_black([r, g, b]: [u8; 3]) -> bool {
    r < 30 && g < 30 && b < 30
}

/// Non-black bars; the frame block alternates white / blue over a few frames.
fn check_picture_changes_per_field(session: &mut CoreSession) {
    let mut seen = BTreeSet::new();
    let mut presented = 0;
    let mut last = RgbaFrameBuffer::default();
    for _ in 0..12 {
        let (video, _) = run(session, HeldInput::None, 1);
        presented += video.presented;
        if video.presented > 0 {
            let [r, g, b] = pixel(&video.last, FRAME_BLOCK.0, FRAME_BLOCK.1);
            seen.insert((r > 128, g > 128, b > 128));
            last = video.last;
        }
    }
    eprintln!(
        "dolphin gl suite: size {:?}, {presented}/12 presented, frame block {seen:?}",
        last.size()
    );
    assert!(presented >= 4, "new frames presented: {presented}");
    let bar = pixel(&last, WHITE_BAR.0, WHITE_BAR.1);
    assert!(
        !is_black(bar) && bar.iter().all(|&c| c > 150),
        "white bar: {bar:?}"
    );
    assert!(
        seen.contains(&(true, true, true)),
        "white frame block: {seen:?}"
    );
    assert!(
        seen.contains(&(false, false, true)),
        "blue frame block: {seen:?}"
    );
}

/// Pad block red, green while A (RetroPad A) is held, red again after.
fn check_pad_block(session: &mut CoreSession) {
    let pad = |session: &mut CoreSession, held| {
        let (video, _) = run(session, held, 6);
        pixel(&video.last, PAD_BLOCK.0, PAD_BLOCK.1)
    };
    let red = pad(session, HeldInput::None);
    assert!(red[0] > 150 && red[1] < 100, "pad block red: {red:?}");
    let green = pad(session, HeldInput::Joypad(8));
    assert!(green[1] > 100 && green[0] < 100, "A held: green {green:?}");
    let back = pad(session, HeldInput::None);
    assert!(back[0] > 150 && back[1] < 100, "released: red {back:?}");
}

/// run_frame cost incl. the readback of each presented frame (logged, not asserted).
fn log_frame_cost(session: &mut CoreSession, spec: &SystemSpec) {
    let mut costs: Vec<Duration> = (0..60)
        .map(|_| {
            let started = Instant::now();
            run(session, HeldInput::None, 1);
            started.elapsed()
        })
        .collect();
    costs.sort();
    eprintln!(
        "dolphin gl suite: {} run_frame + readback p50 {:?} p95 {:?} max {:?}",
        spec.id, costs[30], costs[57], costs[59]
    );
}
