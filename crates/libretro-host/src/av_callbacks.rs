//! Video/audio/input callbacks: translate C arguments into safe slices and
//! forward to the sinks borrowed for the current `run_frame`. Hot path: no
//! allocation, no locks.
//!
//! Dupe frames (`data == NULL`): Snes9x (libretro.cpp @ 1bcc369, 2026-10) never
//! passes NULL; it skips `video_cb` entirely when the frontend disables video
//! via GET_AUDIO_VIDEO_ENABLE (fast-forward skip / run-ahead, `HostContext::av_enable`;
//! frames a core still sends then are discarded here). Other cores may dupe
//! because we answer GET_CAN_DUPE = true, so the path is kept and unit-tested.
//!
//! Hardware-rendered cores pass `RETRO_HW_FRAME_BUFFER_VALID` (`(void*)-1`, pitch 0):
//! the frame is in the session's framebuffer. It is only noted here (never
//! dereferenced) and read back after `retro_run` (`core_session_hw_render`).

use std::ffi::{c_uint, c_void};

use crate::ffi_hw_render_types::RETRO_HW_FRAME_BUFFER_VALID;
use crate::frame_sink_traits::VideoFrame;
use crate::host_context::{with_current, ActiveSinks, HostContext};

fn sinks(ctx: &mut HostContext) -> Option<&mut ActiveSinks> {
    ctx.sinks.as_mut()
}

pub(crate) unsafe extern "C" fn video_refresh(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    with_current((), |ctx| {
        let format = ctx.pixel_format;
        if !ctx.av_enable.video {
            return; // video disabled: the frontend discards it (libretro contract)
        }
        if data as usize == RETRO_HW_FRAME_BUFFER_VALID {
            if ctx.hw_context.is_some() {
                ctx.hw_frame = Some((width, height));
            }
            return; // before any pitch / pointer math
        }
        let Some(sinks) = sinks(ctx) else { return };
        // SAFETY: set by run_frame from a live `&mut` for the duration of retro_run.
        let video = unsafe { &mut *sinks.video };
        if data.is_null() {
            video.dupe(width, height);
            return;
        }
        let row_bytes = width as usize * format.bytes_per_pixel();
        if pitch < row_bytes {
            tracing::warn!(width, pitch, "core frame pitch smaller than row; dropped");
            return;
        }
        let len = match height as usize {
            0 => 0,
            h => pitch * (h - 1) + row_bytes,
        };
        // SAFETY: libretro guarantees `height` rows of `pitch` bytes at `data`
        // (last row at least `row_bytes`), valid until the callback returns.
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) };
        video.frame(&VideoFrame {
            data: bytes,
            width,
            height,
            pitch,
            format,
            bottom_up: false,
        });
    });
}

pub(crate) unsafe extern "C" fn audio_sample(left: i16, right: i16) {
    with_current((), |ctx| {
        if !ctx.av_enable.audio {
            return; // audio disabled (run-ahead hidden/presented frame): discarded
        }
        if let Some(sinks) = sinks(ctx) {
            // SAFETY: see video_refresh.
            unsafe { &mut *sinks.audio }.samples(&[left, right]);
        }
    });
}

pub(crate) unsafe extern "C" fn audio_sample_batch(data: *const i16, frames: usize) -> usize {
    with_current(0, |ctx| {
        if !ctx.av_enable.audio {
            return frames; // discarded, but reported consumed so the core moves on
        }
        // Outside `run_frame` (e.g. Mupen64Plus emulates up to a safe point inside
        // `retro_serialize`) there is no sink: discard, but report it consumed, or a
        // core that loops until its buffer is drained never returns.
        let Some(sinks) = sinks(ctx) else {
            return frames;
        };
        if data.is_null() {
            return 0;
        }
        // SAFETY: core passes `frames` interleaved stereo frames (2 × i16 each).
        let samples = unsafe { std::slice::from_raw_parts(data, frames * 2) };
        // SAFETY: see video_refresh.
        unsafe { &mut *sinks.audio }.samples(samples);
        frames
    })
}

pub(crate) unsafe extern "C" fn input_poll() {
    with_current((), |ctx| {
        if let Some(sinks) = sinks(ctx) {
            // SAFETY: see video_refresh.
            unsafe { &mut *sinks.input }.poll();
        }
    });
}

pub(crate) unsafe extern "C" fn input_state(
    port: c_uint,
    device: c_uint,
    index: c_uint,
    id: c_uint,
) -> i16 {
    with_current(0, |ctx| match sinks(ctx) {
        // SAFETY: see video_refresh.
        Some(sinks) => unsafe { &mut *sinks.input }.state(port, device, index, id),
        None => 0,
    })
}

#[cfg(test)]
#[path = "av_callbacks_tests.rs"]
mod tests;
