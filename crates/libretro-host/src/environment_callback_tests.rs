//! Tests for the `retro_environment` handler (kept apart: the handler is near the size limit).

use super::*;
use crate::host_context::run_with_test_ctx;

fn env(cmd: c_uint, data: *mut c_void) -> bool {
    // SAFETY: tests pass a pointer of the type documented for `cmd`.
    unsafe { environment(cmd, data) }
}

#[test]
fn pixel_format_accepts_known_and_rejects_unknown() {
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            let mut fmt: c_uint = RETRO_PIXEL_FORMAT_RGB565;
            assert!(env(
                RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
                (&raw mut fmt).cast()
            ));
            let mut bad: c_uint = 99;
            assert!(!env(
                RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
                (&raw mut bad).cast()
            ));
        },
    );
    assert_eq!(ctx.pixel_format, PixelFormat::Rgb565);
}

#[test]
fn geometry_change_is_flagged() {
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            let mut geometry = RetroGameGeometry {
                base_width: 512,
                base_height: 448,
                ..Default::default()
            };
            assert!(env(
                RETRO_ENVIRONMENT_SET_GEOMETRY,
                (&raw mut geometry).cast()
            ));
        },
    );
    assert!(ctx.av_changed);
    assert_eq!(ctx.av_info.geometry.base_width, 512);
}

#[test]
fn unknown_command_returns_false_and_no_session_is_safe() {
    run_with_test_ctx(|_| {}, || assert!(!env(9999, std::ptr::null_mut())));
    let mut dupe = false;
    assert!(!env(RETRO_ENVIRONMENT_GET_CAN_DUPE, (&raw mut dupe).cast()));
}

/// `GET_AUDIO_VIDEO_ENABLE` reply for a context prepared by `setup`.
fn av_reply(setup: impl FnOnce(&mut HostContext)) -> c_int {
    let mut flags: c_int = -1;
    run_with_test_ctx(setup, || {
        let cmd = RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE | RETRO_ENVIRONMENT_EXPERIMENTAL;
        assert!(env(cmd, (&raw mut flags).cast()));
    });
    flags
}

#[test]
fn audio_video_enable_reports_each_flag() {
    use crate::host_context::AvEnable;
    let (video, audio) = (RETRO_AV_ENABLE_VIDEO, RETRO_AV_ENABLE_AUDIO);
    assert_eq!(av_reply(|_| {}), video | audio);
    assert_eq!(av_reply(|ctx| ctx.av_enable = AvEnable::NONE), 0);
    let audio_only = AvEnable {
        video: false,
        audio: true,
    };
    assert_eq!(av_reply(|ctx| ctx.av_enable = audio_only), audio);
    let video_only = AvEnable {
        video: true,
        audio: false,
    };
    assert_eq!(av_reply(|ctx| ctx.av_enable = video_only), video);
    // Fast savestates only while a run-ahead state is in progress.
    let fast = av_reply(|ctx| ctx.savestate_context = SavestateContext::RunaheadSameInstance);
    assert_eq!(fast, video | audio | RETRO_AV_ENABLE_FAST_SAVESTATES);
}

#[test]
fn savestate_context_follows_the_session_value() {
    let query = |context| {
        let mut value: c_int = -1;
        run_with_test_ctx(
            |ctx| ctx.savestate_context = context,
            || {
                let cmd = RETRO_ENVIRONMENT_GET_SAVESTATE_CONTEXT | RETRO_ENVIRONMENT_EXPERIMENTAL;
                assert!(env(cmd, std::ptr::null_mut()), "NULL probe is supported");
                assert!(env(cmd, (&raw mut value).cast()));
            },
        );
        value
    };
    assert_eq!(query(SavestateContext::Normal), 0);
    assert_eq!(query(SavestateContext::RunaheadSameInstance), 1);
}

#[test]
fn serialization_quirks_are_stored_and_reply_is_masked() {
    let mut bits: u64 = 0b101 | (1 << 3) | (1 << 50); // INCOMPLETE, CORE_VARIABLE_SIZE, junk
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            let cmd = RETRO_ENVIRONMENT_SET_SERIALIZATION_QUIRKS;
            assert!(env(cmd, (&raw mut bits).cast()));
            assert!(!env(cmd, std::ptr::null_mut()));
        },
    );
    assert_eq!(
        bits,
        0b101 | (1 << 3),
        "unknown bit cleared, FRONT_VARIABLE_SIZE set"
    );
    assert!(ctx.serialization_quirks.incomplete());
    assert!(ctx.serialization_quirks.variable_size());
}
