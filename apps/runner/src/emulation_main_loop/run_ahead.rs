//! Single-instance run-ahead (0–2 frames): hides the game's own input lag by showing
//! the frame N steps in the future, then rolling back to the real timeline.
//!
//! Per presented frame: real frame (video off, audio to the real sink) → serialize →
//! N-1 hidden frames (no AV) → presented frame (video on, audio off) → unserialize.
//! Audio comes only from real frames, so rollback never touches the audio stream.
//! Catch-up ticks (several frames due) run all but the last due frame as plain real
//! frames. Fast-forward and rewind playback bypass this module entirely (their own
//! loops in `fast_forward` / `rewind_playback`), so run-ahead is off while either is held.

use libretro_host::{
    AudioSink, AvEnable, CoreSession, HostError, InputSource, SavestateContext, VideoSink,
};
use system_catalog::SaveStateQuality;

use super::save_state_policy::state_refusal;

mod tick_plan;

pub(crate) use tick_plan::{plan_tick, FrameStep};

/// On-screen notice when run-ahead switches itself off.
pub(crate) const UNAVAILABLE_NOTICE: &str = "Run-ahead unavailable";
/// Pause-menu reason for a run-ahead row disabled by a failure (refusals: see
/// `save_state_policy`).
const STATE_ERROR: &str = "Save state failed this session";

/// What the loop does around real-timeline frames (audio pacing, SRAM flush).
pub(crate) trait RealFrameHooks {
    fn before_real_frame(&mut self);
    /// Sink for real-timeline audio.
    fn audio_sink(&mut self) -> &mut (dyn AudioSink + 'static);
    /// Sink for frames whose audio is discarded anyway (core told audio is off).
    fn muted_sink(&mut self) -> &mut (dyn AudioSink + 'static);
    /// `state`: this frame's end state when run-ahead already serialized it (the
    /// `Real` step), so rewind capture can reuse it instead of serializing again.
    fn after_real_frame(&mut self, session: &mut CoreSession, state: Option<&[u8]>);
}

/// Run-ahead state for one session.
pub(crate) struct RunAhead {
    frames: u8,
    /// Reused rollback state (no per-frame allocation after the first).
    buf: Vec<u8>,
    /// Why run-ahead is off for this session (set once it gave up or was refused).
    unavailable: Option<&'static str>,
    /// The rollback state of the current tick was taken.
    captured: bool,
    /// A serialize succeeded once: until then real frames keep video on, so a core
    /// that is not ready yet (`MUST_INITIALIZE`) still shows a fresh picture.
    primed: bool,
    notice: Option<&'static str>,
}

impl RunAhead {
    /// `frames` from `latency.run_ahead_frames`; refused for cores whose states are
    /// incomplete or not frame-exact (`state_refusal`; silently when off: the pause
    /// menu shows the reason).
    pub(crate) fn new(frames: u8, session: &CoreSession, quality: SaveStateQuality) -> Self {
        let mut run_ahead = Self {
            frames,
            buf: Vec::new(),
            unavailable: None,
            captured: false,
            primed: false,
            notice: None,
        };
        let incomplete = session.serialization_quirks().incomplete();
        if let Some(reason) = state_refusal(incomplete, quality) {
            if frames > 0 {
                run_ahead.disable(reason, reason);
            }
            run_ahead.unavailable = Some(reason);
        }
        run_ahead
    }

    /// Frames ahead in effect (0 when off or disabled).
    pub(crate) fn active_frames(&self) -> u8 {
        if self.unavailable.is_some() {
            0
        } else {
            self.frames
        }
    }

    /// Pause-menu change; takes effect on the next tick (no-op while unavailable).
    pub(crate) fn set_frames(&mut self, frames: u8) {
        self.frames = frames;
        self.captured = false;
    }

    /// Why run-ahead cannot be used this session, if it cannot.
    pub(crate) fn unavailable_reason(&self) -> Option<&'static str> {
        self.unavailable
    }

    /// On-screen message to show once (run-ahead just switched itself off).
    pub(crate) fn take_notice(&mut self) -> Option<&'static str> {
        self.notice.take()
    }

    /// Emulates one tick with `due` frames due.
    pub(crate) fn run_tick(
        &mut self,
        due: u32,
        session: &mut CoreSession,
        video: &mut (dyn VideoSink + 'static),
        input: &mut (dyn InputSource + 'static),
        hooks: &mut impl RealFrameHooks,
    ) -> anyhow::Result<()> {
        for step in plan_tick(due, self.active_frames()) {
            let real = step.is_real_timeline();
            if real {
                hooks.before_real_frame();
            }
            let audio = if real {
                hooks.audio_sink()
            } else {
                hooks.muted_sink()
            };
            self.run_step(step, session, video, audio, input)?;
            if real {
                let state = (step == FrameStep::Real && self.captured).then_some(&self.buf[..]);
                hooks.after_real_frame(session, state);
            }
        }
        Ok(())
    }

    fn run_step(
        &mut self,
        step: FrameStep,
        session: &mut CoreSession,
        video: &mut (dyn VideoSink + 'static),
        audio: &mut (dyn AudioSink + 'static),
        input: &mut (dyn InputSource + 'static),
    ) -> anyhow::Result<()> {
        let av = match step {
            FrameStep::Plain => AvEnable::ALL,
            FrameStep::CatchUp => AUDIO_ONLY,
            FrameStep::Real if self.primed => AUDIO_ONLY,
            FrameStep::Real => AvEnable::ALL,
            // Nothing captured (not ready / just disabled): the real frame stands.
            FrameStep::Hidden | FrameStep::Presented if !self.captured => return Ok(()),
            FrameStep::Hidden => AvEnable::NONE,
            FrameStep::Presented => VIDEO_ONLY,
        };
        session.set_av_enable(av);
        let ran = session.run_frame(video, audio, input);
        session.set_av_enable(AvEnable::ALL);
        ran?;
        match step {
            FrameStep::Real => self.capture(session),
            FrameStep::Presented => self.roll_back(session),
            _ => {}
        }
        Ok(())
    }

    /// After the real frame: keep its state for the rollback.
    fn capture(&mut self, session: &mut CoreSession) {
        match session.serialize_into(&mut self.buf, SavestateContext::RunaheadSameInstance) {
            Ok(_) => (self.captured, self.primed) = (true, true),
            Err(HostError::SerializeNotReady) => {} // retried next tick
            Err(err) => self.disable(STATE_ERROR, &format!("serialize failed: {err}")),
        }
    }

    /// After the presented frame: back to the real timeline.
    fn roll_back(&mut self, session: &mut CoreSession) {
        self.captured = false;
        let restored = session.unserialize_from(&self.buf, SavestateContext::RunaheadSameInstance);
        if let Err(err) = restored {
            // Emulation continues from the run-ahead frame (N frames early, once).
            self.disable(STATE_ERROR, &format!("unserialize failed: {err}"));
        }
    }

    fn disable(&mut self, reason: &'static str, detail: &str) {
        tracing::warn!(detail, "run-ahead disabled for this session");
        (self.unavailable, self.captured) = (Some(reason), false);
        self.notice = Some(UNAVAILABLE_NOTICE);
    }
}

const AUDIO_ONLY: AvEnable = AvEnable {
    video: false,
    audio: true,
};
/// Picture only (run-ahead's presented frame, rewind playback).
pub(crate) const VIDEO_ONLY: AvEnable = AvEnable {
    video: true,
    audio: false,
};
