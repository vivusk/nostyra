//! Tests for the AV/input callbacks (kept apart to keep the callbacks file small).

use super::*;
use crate::frame_sink_traits::*;
use crate::host_context::{run_with_test_ctx, AvEnable};

/// Runs `f` with null sinks installed; returns them for assertions.
fn with_sinks(f: impl FnOnce()) -> (NullVideoSink, NullAudioSink) {
    let mut video = NullVideoSink::default();
    let mut audio = NullAudioSink::default();
    let mut input = NullInputSource;
    let (v, a, i) = (
        &raw mut video as *mut (dyn VideoSink + 'static),
        &raw mut audio as *mut (dyn AudioSink + 'static),
        &raw mut input as *mut (dyn InputSource + 'static),
    );
    run_with_test_ctx(
        |ctx| {
            ctx.pixel_format = PixelFormat::Rgb565;
            ctx.sinks = Some(ActiveSinks {
                video: v,
                audio: a,
                input: i,
            });
        },
        f,
    );
    (video, audio)
}

#[test]
fn null_frame_is_forwarded_as_dupe() {
    // SAFETY: NULL data is the documented dupe signal; no memory is read.
    let (video, _) = with_sinks(|| unsafe { video_refresh(std::ptr::null(), 256, 224, 512) });
    assert_eq!((video.dupes, video.frames), (1, 0));
    assert_eq!(video.last_size, (256, 224));
}

#[test]
fn frame_with_padding_hashes_visible_pixels_only() {
    // 2×2 RGB565 frame, pitch 8 (4 bytes padding per row).
    let a = [1u8, 2, 3, 4, 0xAA, 0xAA, 0xAA, 0xAA, 5, 6, 7, 8];
    let b = [1u8, 2, 3, 4, 0x55, 0x55, 0x55, 0x55, 5, 6, 7, 8];
    // SAFETY: buffers hold (h-1)*pitch + w*2 bytes as required.
    let (va, _) = with_sinks(|| unsafe { video_refresh(a.as_ptr().cast(), 2, 2, 8) });
    // SAFETY: as above.
    let (vb, _) = with_sinks(|| unsafe { video_refresh(b.as_ptr().cast(), 2, 2, 8) });
    assert_eq!(va.frames, 1);
    assert_eq!(va.last_checksum, vb.last_checksum);
}

#[test]
fn audio_batch_counts_stereo_frames() {
    let samples = [100i16, -200, 300, -400];
    let (_, audio) = with_sinks(|| {
        // SAFETY: 2 stereo frames = 4 samples.
        let consumed = unsafe { audio_sample_batch(samples.as_ptr(), 2) };
        assert_eq!(consumed, 2);
    });
    assert_eq!((audio.stereo_frames, audio.peak), (2, 400));
}

#[test]
fn disabled_outputs_never_reach_the_sinks() {
    let pixels = [0u8; 8];
    let samples = [1i16, 2];
    let (mut video, mut audio) = (NullVideoSink::default(), NullAudioSink::default());
    let mut input = NullInputSource;
    let (v, a, i) = (
        &raw mut video as *mut (dyn VideoSink + 'static),
        &raw mut audio as *mut (dyn AudioSink + 'static),
        &raw mut input as *mut (dyn InputSource + 'static),
    );
    for av in [
        AvEnable {
            video: false,
            audio: true,
        },
        AvEnable {
            video: true,
            audio: false,
        },
    ] {
        run_with_test_ctx(
            |ctx| {
                ctx.av_enable = av;
                ctx.pixel_format = PixelFormat::Rgb565;
                ctx.sinks = Some(ActiveSinks {
                    video: v,
                    audio: a,
                    input: i,
                });
            },
            // SAFETY: 2×2 RGB565 frame (8 bytes, pitch 4); one stereo frame.
            || unsafe {
                video_refresh(pixels.as_ptr().cast(), 2, 2, 4);
                assert_eq!(audio_sample_batch(samples.as_ptr(), 1), 1);
                audio_sample(3, 4);
            },
        );
    }
    // Each output arrived exactly once: in the pass where it was enabled.
    assert_eq!((video.frames, audio.stereo_frames), (1, 2));
}

#[test]
fn audio_batch_without_sinks_is_reported_consumed() {
    // Mupen64Plus pushes audio while `retro_serialize` advances the emulation (no
    // `run_frame`, no sinks) and loops until every frame is accepted.
    let samples = [1i16, 2, 3, 4, 5, 6];
    run_with_test_ctx(
        |_| {},
        // SAFETY: 3 stereo frames = 6 samples.
        || assert_eq!(unsafe { audio_sample_batch(samples.as_ptr(), 3) }, 3),
    );
}
