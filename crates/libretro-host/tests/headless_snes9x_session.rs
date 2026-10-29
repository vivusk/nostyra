//! Headless end-to-end test: real snes9x core + the repo's test ROM, null sinks.
//!
//! Requires the core fetched by `scripts/fetch-snes9x-core.{sh,ps1}` (CI does this).
//! Everything runs in ONE test fn: a process may host only one core session, and
//! the test harness would otherwise run tests on parallel threads.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use common::{core_path, load, repo_path, Sinks};
use libretro_host::{probe_core, HostError, PixelFormat};

#[test]
fn snes9x_session_end_to_end() {
    // Surfaces core logs (via the C log shim) in test output on failure.
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();
    let core = core_path();
    let mut session = load(&core).expect("load core + test ROM");

    assert_eq!(session.core_info().name, "Snes9x");
    assert!(!session.core_info().version.is_empty());

    // AV info: NTSC SNES ≈ 60.1 fps, ≈ 32040.5 Hz.
    let av = session.av_info();
    assert!((av.timing.fps - 60.1).abs() < 0.2, "fps {}", av.timing.fps);
    assert!(
        (av.timing.sample_rate - 32040.0).abs() < 100.0,
        "rate {}",
        av.timing.sample_rate
    );

    // Simulate an empty .srm being loaded; the ROM bumps byte 0 on every boot.
    let sram = session.save_ram_mut().expect("test ROM has battery SRAM");
    assert_eq!(sram.len(), 2048);
    sram[0] = 0;

    let mut sinks = Sinks::new();
    let checksums = sinks.run(&mut session, 300);
    assert_eq!(session.pixel_format(), PixelFormat::Rgb565);
    assert_eq!(sinks.video.frames, 300, "one frame per retro_run");
    assert!(
        matches!(sinks.video.last_size, (256, 224) | (512, 448)),
        "{:?}",
        sinks.video.last_size
    );
    let expected_audio = 300.0 * av.timing.sample_rate / av.timing.fps;
    let audio = sinks.audio.stereo_frames as f64;
    assert!(
        (audio / expected_audio - 1.0).abs() < 0.05,
        "audio frames {audio} vs {expected_audio}"
    );
    assert!(sinks.audio.peak > 1000, "test ROM tone should be audible");
    assert!(
        checksums.windows(2).any(|w| w[0] != w[1]),
        "frames should change over time"
    );
    assert_eq!(session.save_ram_mut().unwrap()[0], 1, "SRAM boot counter");

    // Save state round trip: replaying from a state reproduces identical frames.
    let state = session.serialize().expect("serialize");
    let first = sinks.run(&mut session, 30);
    session.unserialize(&state).expect("unserialize");
    let replay = sinks.run(&mut session, 30);
    assert_eq!(
        first, replay,
        "emulation must be deterministic from a save state"
    );

    // Frame skip (fast-forward): no video reaches the sink, emulation state is unchanged,
    // and the frame after the skipped ones matches a normal run.
    session.unserialize(&state).expect("unserialize");
    let normal = sinks.run(&mut session, 31);
    session.unserialize(&state).expect("unserialize");
    let frames_before = sinks.video.frames;
    session.set_frame_skip(true);
    let started = std::time::Instant::now();
    sinks.run(&mut session, 30);
    let skipped_time = started.elapsed();
    session.set_frame_skip(false);
    assert_eq!(
        sinks.video.frames, frames_before,
        "skipped frames are discarded"
    );
    let after_skip = sinks.run(&mut session, 1);
    assert_eq!(after_skip[0], normal[30], "skip must not change emulation");
    eprintln!("30 skipped frames in {skipped_time:?}");

    // One core per process.
    assert!(matches!(load(&core), Err(HostError::SessionAlreadyActive)));

    // Drop fully tears down (unload + deinit + slot release): a fresh session works.
    drop(session);
    let mut again = load(&core).expect("reload after drop");
    Sinks::new().run(&mut again, 1);
    drop(again);

    // Bad core path is a core error (runner exit code 3).
    let err = load(&repo_path("cores/does-not-exist.bin"))
        .err()
        .expect("must fail");
    assert!(err.is_core_error(), "{err}");

    // Probe (no session, no retro_init): identity of a real core, core error otherwise.
    let info = probe_core(&core).expect("probe snes9x");
    assert_eq!(info.name, "Snes9x");
    assert!(!info.version.is_empty());
    let err = probe_core(&repo_path("cores/does-not-exist.bin")).expect_err("must fail");
    assert!(err.is_core_error(), "{err}");
}
