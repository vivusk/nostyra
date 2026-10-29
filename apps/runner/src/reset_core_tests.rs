//! Pause-menu "Reset game" at core level against a real core (snes9x, mesen, mgba,
//! genesis_plus_gx, mupen64plus-next) + test ROM:
//! `CoreSession::reset` keeps battery SRAM and emulation continues from boot.
//! Called from `runner_real_core_suite` (one core session per process). The glue
//! (SRAM flush, rewind clear, audio, OSD) is checked manually.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use libretro_host::{CoreSession, NullAudioSink, NullInputSource, NullVideoSink};

fn run(session: &mut CoreSession, frames: usize) -> Vec<u64> {
    let (mut video, mut audio, mut input) = (
        NullVideoSink::default(),
        NullAudioSink::default(),
        NullInputSource,
    );
    (0..frames)
        .map(|_| {
            session
                .run_frame(&mut video, &mut audio, &mut input)
                .unwrap();
            video.last_checksum
        })
        .collect()
}

/// Run → snapshot SRAM → reset (between `retro_run` calls, as the paused menu
/// does) → SRAM untouched by the reset itself; frames keep running and the ROM's
/// boot code runs again (it bumps SRAM byte 0 once per boot, the rest stays).
pub(crate) fn check_reset(session: &mut CoreSession) {
    run(session, 60);
    let before = session.save_ram_mut().expect("test ROM has SRAM").to_vec();

    session.reset();
    let after_reset = session.save_ram_mut().unwrap().to_vec();
    assert_eq!(after_reset, before, "reset keeps battery SRAM");

    // 120: Mupen64Plus-Next resets with a delayed NMI (the boot code reruns ~50
    // frames later); the other cores reboot at once.
    let frames = run(session, 120);
    assert!(
        frames.windows(2).any(|w| w[0] != w[1]),
        "frames keep changing after the reset"
    );
    let after = session.save_ram_mut().unwrap().to_vec();
    assert_eq!(
        after[0],
        before[0].wrapping_add(1),
        "boot code ran again (boot counter +1)"
    );
    assert_eq!(after[1..], before[1..], "the rest of SRAM is untouched");
}
