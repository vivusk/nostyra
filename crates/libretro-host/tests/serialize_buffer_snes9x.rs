//! `serialize_into` / `unserialize_from` (run-ahead path) and the split AV flags on
//! the real snes9x core. Own test binary: one core session per process.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use common::{core_path, load, Sinks};
use libretro_host::{AvEnable, SavestateContext};

#[test]
fn snes9x_serialize_buffer_and_av_flags() {
    let mut session = load(&core_path()).expect("load core + test ROM");
    let mut sinks = Sinks::new();
    sinks.run(&mut session, 60);
    let state = session.serialize().expect("serialize");
    let first = sinks.run(&mut session, 30);

    // serialize_into reuses the caller's buffer (no reallocation) and matches serialize().
    let mut buf = Vec::new();
    let ctx = SavestateContext::RunaheadSameInstance;
    session.unserialize(&state).expect("unserialize");
    assert_eq!(session.serialize_into(&mut buf, ctx).unwrap(), state.len());
    assert_eq!(buf, state);
    let (ptr, capacity) = (buf.as_ptr(), buf.capacity());
    for _ in 0..3 {
        sinks.run(&mut session, 1);
        session.serialize_into(&mut buf, ctx).unwrap();
    }
    assert_eq!((buf.as_ptr(), buf.capacity()), (ptr, capacity));
    session
        .unserialize_from(&state, ctx)
        .expect("unserialize_from");
    assert_eq!(
        sinks.run(&mut session, 30),
        first,
        "context does not change the state"
    );
    assert!(
        !session.serialization_quirks().incomplete(),
        "snes9x states are complete"
    );

    // Split AV flags: video and audio are dropped independently.
    let (frames, audio) = (sinks.video.frames, sinks.audio.stereo_frames);
    session.set_av_enable(AvEnable {
        video: false,
        audio: true,
    });
    sinks.run(&mut session, 2);
    assert_eq!(sinks.video.frames, frames, "video off");
    assert!(sinks.audio.stereo_frames > audio, "audio on");
    let audio = sinks.audio.stereo_frames;
    session.set_av_enable(AvEnable {
        video: true,
        audio: false,
    });
    sinks.run(&mut session, 2);
    assert_eq!(sinks.video.frames, frames + 2, "video on");
    assert_eq!(sinks.audio.stereo_frames, audio, "audio off");
    session.set_av_enable(AvEnable::ALL);
}
