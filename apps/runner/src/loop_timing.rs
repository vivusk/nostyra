//! Decides how many frames to emulate per redraw and keeps audio in sync
//! (system-architecture §3.1): vsync mode, audio-sync mode, or wall-clock
//! fallback when no audio device is available.

use std::time::{Duration, Instant};

use audio_output::{select_sync_mode, SdlAudioOutput, SyncMode};
use libretro_host::{AudioSink, AvInfo, NullAudioSink};
use sdl3::AudioSubsystem;
use video_renderer::PresentMode;

use crate::frame_pacer::{FpsMeter, FramePacer};

/// Audio queue target; low enough for responsive play, high enough for jitter.
const TARGET_LATENCY: Duration = Duration::from_millis(64);
/// Longest audio-sync wait per frame, so a stalled device can't freeze the UI.
const MAX_AUDIO_WAIT: Duration = Duration::from_millis(100);
/// Emulated frames between audio stats logs (~5 s).
const STATS_EVERY_FRAMES: u32 = 300;

pub struct LoopTiming {
    audio: Option<SdlAudioOutput>,
    null_audio: NullAudioSink,
    pacer: FramePacer,
    fps_meter: FpsMeter,
    mode: Option<SyncMode>,
    av: AvInfo,
    force_audio_sync: bool,
    frames_since_stats: u32,
    /// Frames skipped by the queue-overflow guard since start.
    overruns: u64,
}

impl LoopTiming {
    pub fn new(audio: Option<&AudioSubsystem>, av: AvInfo, force_audio_sync: bool) -> Self {
        let audio = audio.and_then(|a| {
            SdlAudioOutput::open(a, av.timing.sample_rate, TARGET_LATENCY)
                .inspect_err(|err| tracing::warn!(%err, "no audio output; wall-clock pacing"))
                .ok()
        });
        Self {
            audio,
            null_audio: NullAudioSink::default(),
            pacer: FramePacer::new(av.timing.fps),
            fps_meter: FpsMeter::new(),
            mode: None,
            av,
            force_audio_sync,
            frames_since_stats: 0,
            overruns: 0,
        }
    }

    /// (Re)selects the sync mode for `display_hz`; returns present-mode preferences.
    pub fn select_mode(&mut self, display_hz: Option<f64>) -> &'static [PresentMode] {
        let Some(audio) = self.audio.as_mut() else {
            return &[PresentMode::Fifo]; // wall-clock pacer; vsync just limits redraws
        };
        let (fps, rate) = (self.av.timing.fps, self.av.timing.sample_rate);
        let mode = select_sync_mode(display_hz, fps, rate, self.force_audio_sync);
        audio.set_base_ratio(mode.base_ratio());
        self.mode = Some(mode);
        tracing::info!(?display_hz, ?mode, "sync mode selected");
        match mode {
            SyncMode::Vsync { .. } => &[PresentMode::Fifo],
            SyncMode::AudioSync { .. } => &[
                PresentMode::Immediate,
                PresentMode::Mailbox,
                PresentMode::Fifo,
            ],
        }
    }

    /// Frames to emulate this redraw. Audio-sync mode blocks until the audio
    /// queue has room; a queue far above target (e.g. window occluded, so vsync
    /// no longer throttles) skips the frame instead of growing latency.
    pub fn frames_due(&mut self, now: Instant) -> u32 {
        let due = match (&self.audio, self.mode) {
            (Some(audio), _) if audio.queued_frames() > 2.0 * self.target_frames() => {
                self.overruns += 1;
                0
            }
            (Some(audio), Some(SyncMode::AudioSync { .. })) => {
                let deadline = Instant::now() + MAX_AUDIO_WAIT;
                while audio.is_above_target() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(1));
                }
                1
            }
            (Some(_), _) => 1,
            (None, _) => self.pacer.frames_due(now),
        };
        self.fps_meter.record(now, due);
        due
    }

    fn target_frames(&self) -> f64 {
        self.av.timing.sample_rate.round() * TARGET_LATENCY.as_secs_f64()
    }

    /// Sink for this frame's samples.
    pub fn audio_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        match self.audio.as_mut() {
            Some(audio) => audio,
            None => &mut self.null_audio,
        }
    }

    /// Silent sink for fast-forward frames (their audio would only overflow the queue).
    pub fn muted_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        &mut self.null_audio
    }

    /// Fast-forward ended: forget the wall-clock backlog so pacing restarts cleanly and
    /// refill the drained audio queue (otherwise vsync mode underruns for seconds).
    pub fn resume_normal_speed(&mut self) {
        self.pacer.reset();
        if let Some(audio) = self.audio.as_mut() {
            audio.restart_prefill();
        }
    }

    /// Before each emulated frame: samples the pre-push queue for underruns.
    pub fn before_frame(&mut self) {
        if let Some(audio) = self.audio.as_mut() {
            audio.before_frame();
        }
    }

    /// After each emulated frame: DRC update.
    pub fn after_frame(&mut self) {
        if let Some(audio) = self.audio.as_mut() {
            let stats = audio.update();
            self.frames_since_stats += 1;
            if self.frames_since_stats >= STATS_EVERY_FRAMES {
                self.frames_since_stats = 0;
                tracing::debug!(
                    underruns = stats.underruns,
                    overruns = self.overruns,
                    fill_ms = format_args!(
                        "{:.1}",
                        1000.0 * stats.fill_frames / self.av.timing.sample_rate
                    ),
                    min_fill_ms = format_args!(
                        "{:.1}",
                        1000.0 * stats.min_fill_frames / self.av.timing.sample_rate
                    ),
                    ratio = format_args!("{:.5}", stats.ratio),
                    "audio stats"
                );
                audio.reset_min_fill();
            }
        }
    }

    /// Core changed timing (SET_SYSTEM_AV_INFO): reopen audio if the rate changed.
    /// Returns the new present-mode preferences; the caller must apply them.
    pub fn on_av_change(
        &mut self,
        av: AvInfo,
        audio: Option<&AudioSubsystem>,
        display_hz: Option<f64>,
    ) -> &'static [PresentMode] {
        let rate_changed = av.timing.sample_rate.round() != self.av.timing.sample_rate.round();
        self.av = av;
        self.pacer.set_fps(av.timing.fps);
        if rate_changed {
            *self = Self::new(audio, av, self.force_audio_sync);
        }
        self.select_mode(display_hz)
    }

    /// Whether a present mode can pace audio-sync correctly (see §3.1).
    pub fn warn_if_fifo_throttles(&self, present_mode: PresentMode, display_hz: Option<f64>) {
        let audio_sync = matches!(self.mode, Some(SyncMode::AudioSync { .. }));
        let slow_display = display_hz.is_none_or(|hz| hz <= self.av.timing.fps);
        if audio_sync && present_mode == PresentMode::Fifo && slow_display {
            tracing::warn!(
                ?display_hz,
                "Fifo on a display slower than the core may slow emulation"
            );
        }
    }
}
