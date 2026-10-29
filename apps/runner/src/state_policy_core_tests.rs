//! Run-ahead and rewind refuse a core whose save states are not frame-exact
//! (`SaveStateQuality::Serialized`), with the pause-menu reason, whether or not they
//! are switched on. Called from the real-core suites (snes9x with the quality forced;
//! the N64 suite with its registry quality).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use config_layers::RewindConfig;
use libretro_host::{CoreSession, NullAudioSink, NullInputSource, NullVideoSink};
use system_catalog::SaveStateQuality;

use crate::emulation_main_loop::rewind_playback::Rewind;
use crate::emulation_main_loop::run_ahead::{RunAhead, UNAVAILABLE_NOTICE};
use crate::emulation_main_loop::save_state_policy::STATES_NOT_FRAME_EXACT;

/// `quality` must be `Serialized` (the core's registry value, or forced).
pub(crate) fn check_states_refused(session: &mut CoreSession, quality: SaveStateQuality) {
    let reason = Some(STATES_NOT_FRAME_EXACT);
    for frames in [0, 2] {
        let mut run_ahead = RunAhead::new(frames, session, quality);
        assert_eq!(run_ahead.active_frames(), 0, "run-ahead {frames} refused");
        assert_eq!(run_ahead.unavailable_reason(), reason);
        let notice = (frames > 0).then_some(UNAVAILABLE_NOTICE);
        assert_eq!(
            run_ahead.take_notice(),
            notice,
            "notice only when it was on"
        );
    }
    for enabled in [false, true] {
        let config = RewindConfig {
            enabled,
            buffer_mb: 64,
        };
        let mut rewind = Rewind::new(&config, session, quality);
        assert_eq!(rewind.unavailable_reason(), reason, "rewind on: {enabled}");
        let (mut video, mut audio) = (NullVideoSink::default(), NullAudioSink::default());
        for _ in 0..4 {
            session
                .run_frame(&mut video, &mut audio, &mut NullInputSource)
                .unwrap();
            rewind.after_real_frame(session, None, || None);
        }
        assert_eq!(rewind.pop_captures(1), None, "nothing captured");
        assert_eq!(rewind.set_held(true), None, "held rewind does not start");
        assert!(!rewind.is_rewinding());
        let notice = enabled.then_some("Rewind unavailable");
        assert_eq!(rewind.take_notice(), notice, "notice only when it was on");
    }
}
