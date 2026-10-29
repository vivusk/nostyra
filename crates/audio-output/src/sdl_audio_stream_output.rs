//! `AudioSink` backed by an SDL3 `AudioStream` on the default playback device.
//!
//! SDL resamples from `round(core_rate)` to the device rate; we only steer the
//! stream's frequency ratio (DRC) once per frame. The device stays paused until
//! the queue first reaches the target fill, so playback starts without an underrun.

use std::time::Duration;

use libretro_host::AudioSink;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};
use sdl3::AudioSubsystem;

use crate::dynamic_rate_control::{drc_ratio, DEFAULT_MAX_DEVIATION};

/// Bytes per stereo i16 frame.
const FRAME_BYTES: f64 = 4.0;

/// Failures opening or driving the SDL audio stream.
#[derive(Debug, thiserror::Error)]
pub enum AudioOutputError {
    #[error("SDL audio error: {0}")]
    Sdl(#[from] sdl3::Error),
}

/// Running counters for diagnostics (logged by the runner).
#[derive(Clone, Copy, Debug, Default)]
pub struct AudioStats {
    /// Times the queue ran empty after playback started.
    pub underruns: u64,
    /// Last measured queue fill in stereo frames (source rate), after the push.
    pub fill_frames: f64,
    /// Lowest pre-push fill since the last `reset_min_fill` (the sawtooth trough).
    pub min_fill_frames: f64,
    pub ratio: f64,
}

pub struct SdlAudioOutput {
    stream: AudioStreamOwner,
    /// Target queue fill in stereo frames at the source rate.
    target_frames: f64,
    base_ratio: f64,
    playing: bool,
    /// True while the queue is empty, so one starvation counts once (edge).
    starved: bool,
    stats: AudioStats,
}

impl SdlAudioOutput {
    /// Opens the default playback device for int16 stereo at `round(core_rate)`.
    pub fn open(
        audio: &AudioSubsystem,
        core_rate: f64,
        target_latency: Duration,
    ) -> Result<Self, AudioOutputError> {
        let src_rate = core_rate.round();
        let spec = AudioSpec {
            freq: Some(src_rate as i32),
            channels: Some(2),
            format: Some(AudioFormat::s16_sys()),
        };
        let stream = audio
            .default_playback_device()
            .open_device_stream(Some(&spec))?;
        tracing::info!(src_rate, device = ?stream.device_name(), "audio stream opened");
        Ok(Self {
            stream,
            target_frames: src_rate * target_latency.as_secs_f64(),
            base_ratio: 1.0,
            playing: false,
            starved: false,
            stats: AudioStats {
                min_fill_frames: f64::INFINITY,
                ..AudioStats::default()
            },
        })
    }

    /// Sets the ratio used at target fill (from the active `SyncMode`).
    pub fn set_base_ratio(&mut self, base_ratio: f64) {
        self.base_ratio = base_ratio;
    }

    /// Queued input in stereo frames (what the device has not consumed yet).
    pub fn queued_frames(&self) -> f64 {
        f64::from(self.stream.queued_bytes().unwrap_or(0).max(0)) / FRAME_BYTES
    }

    /// True while more than the target is queued: audio-sync mode waits on this.
    pub fn is_above_target(&self) -> bool {
        self.queued_frames() > self.target_frames
    }

    /// Call right BEFORE the core pushes a frame's samples: the queue is at its
    /// trough here, so an empty queue means the device really starved.
    pub fn before_frame(&mut self) {
        if !self.playing {
            return;
        }
        let fill = self.queued_frames();
        self.stats.min_fill_frames = self.stats.min_fill_frames.min(fill);
        if fill == 0.0 {
            if !self.starved {
                self.stats.underruns += 1;
                tracing::debug!(underruns = self.stats.underruns, "audio underrun");
            }
            self.starved = true;
        } else {
            self.starved = false;
        }
    }

    /// Starts a new min-fill measurement window (after logging stats).
    pub fn reset_min_fill(&mut self) {
        self.stats.min_fill_frames = f64::INFINITY;
    }

    /// Drops queued audio and pauses the device so the next `update` prefills to the
    /// target again (after a stretch where nothing was pushed, e.g. fast-forward).
    pub fn restart_prefill(&mut self) {
        if let Err(err) = self.stream.clear().and_then(|()| self.stream.pause()) {
            tracing::warn!(%err, "cannot restart audio prefill");
        }
        self.playing = false;
        self.starved = false;
    }

    /// Call once per emulated frame, after the push: starts playback after
    /// prefill and applies the DRC ratio.
    pub fn update(&mut self) -> AudioStats {
        let fill = self.queued_frames();
        if !self.playing && fill >= self.target_frames {
            if let Err(err) = self.stream.resume() {
                tracing::warn!(%err, "cannot start audio device");
            }
            self.playing = true;
        }
        let ratio = drc_ratio(
            fill,
            self.target_frames,
            self.base_ratio,
            DEFAULT_MAX_DEVIATION,
        );
        set_frequency_ratio(&self.stream, ratio as f32);
        self.stats.fill_frames = fill;
        self.stats.ratio = ratio;
        self.stats
    }
}

impl AudioSink for SdlAudioOutput {
    fn samples(&mut self, interleaved: &[i16]) {
        // SDL copies into its own queue; nothing allocated on our side.
        if let Err(err) = self.stream.put_data_i16(interleaved) {
            tracing::warn!(%err, "dropping audio samples");
        }
    }
}

/// `SDL_SetAudioStreamFrequencyRatio` is not wrapped by the `sdl3` crate (0.20).
#[allow(unsafe_code)] // direct sdl3::sys call the safe wrapper lacks (code standards)
fn set_frequency_ratio(stream: &AudioStreamOwner, ratio: f32) {
    // SAFETY: `raw()` is the live stream owned by `stream` for this call; SDL
    // validates the ratio range (0.01..=100) and returns false otherwise.
    let ok = unsafe { sdl3::sys::audio::SDL_SetAudioStreamFrequencyRatio(stream.raw(), ratio) };
    if !ok {
        tracing::warn!(ratio, "SDL rejected audio frequency ratio");
    }
}
