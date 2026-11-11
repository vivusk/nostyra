//! Answers ggrs requests with a real core: `SaveGameState` → `serialize_into`
//! (`RunaheadSameInstance`) + desync checksum, `LoadGameState` → `unserialize_from`,
//! `AdvanceFrame` → one muted `run_frame` fed from the frame's inputs. Records the
//! numbers the spike report needs (timings, advances per frame, RAM CRC per frame).

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::time::Instant;

use anyhow::Context;
use ggrs::{Frame, GgrsRequest};
use libretro_host::{AvEnable, CoreSession, NullAudioSink, NullVideoSink, SavestateContext};

use super::frame_inputs_source::FrameInputsSource;
use super::spike_config::SpikeConfig;

const CONTEXT: SavestateContext = SavestateContext::RunaheadSameInstance;

/// Where the desync checksum came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChecksumSource {
    /// crc32 over `system_ram()` + SRAM.
    SystemRam,
    /// The core exposes no system RAM: crc32 over the state blob.
    StateBlob,
}

/// crc32 over the core's system RAM + SRAM, or `None` without system RAM.
pub(crate) fn ram_crc(session: &mut CoreSession) -> Option<u32> {
    let mut hasher = crc32fast::Hasher::new();
    let ram = session.system_ram().filter(|ram| !ram.is_empty())?;
    hasher.update(ram);
    if let Some(sram) = session.save_ram_mut() {
        hasher.update(sram);
    }
    Some(hasher.finalize())
}

/// One core driven by ggrs requests.
#[derive(Default)]
pub(crate) struct CoreRollbackGame {
    buf: Vec<u8>,
    pub input: FrameInputsSource,
    video: NullVideoSink,
    audio: NullAudioSink,
    /// Frame the core is at (= next frame `AdvanceFrame` produces).
    pub frame: Frame,
    /// `AdvanceFrame` calls per frame number (F20 re-advance counts).
    pub advances: Vec<u8>,
    pub serialize_ms: Vec<f64>,
    pub unserialize_ms: Vec<f64>,
    pub source: Option<ChecksumSource>,
    /// RAM crc right after each advance, keyed by the frame it produced (last write wins).
    pub crc_after: HashMap<Frame, u32>,
    pub loads: u32,
    /// Also render video and fold the frame hash into the checksum (bits 32..96):
    /// the test ROMs show input on screen, not in RAM.
    pub video_in_checksum: bool,
}

impl CoreRollbackGame {
    /// Handles one `advance_frame` worth of requests in order.
    pub(crate) fn handle<A>(
        &mut self,
        session: &mut CoreSession,
        requests: Vec<GgrsRequest<SpikeConfig<A>>>,
    ) -> anyhow::Result<()>
    where
        A: Clone + PartialEq + Eq + Hash + Debug + 'static,
    {
        for request in requests {
            match request {
                GgrsRequest::SaveGameState { cell, frame } => {
                    anyhow::ensure!(frame == self.frame, "save {frame} at {}", self.frame);
                    let started = Instant::now();
                    session.serialize_into(&mut self.buf, CONTEXT)?;
                    self.serialize_ms
                        .push(started.elapsed().as_secs_f64() * 1e3);
                    let checksum = self.checksum(session);
                    cell.save(frame, Some((self.buf.clone(), checksum)), Some(checksum));
                }
                GgrsRequest::LoadGameState { cell, frame } => {
                    let (state, checksum) = cell.load().context("ggrs cell without a state")?;
                    self.video.last_checksum = (checksum >> 32) as u64; // frame shown at that state
                    let started = Instant::now();
                    session.unserialize_from(&state, CONTEXT)?;
                    self.unserialize_ms
                        .push(started.elapsed().as_secs_f64() * 1e3);
                    self.frame = frame;
                    self.loads += 1;
                }
                GgrsRequest::AdvanceFrame { inputs } => {
                    self.input
                        .set_inputs(inputs.into_iter().map(|(input, _)| input));
                    self.run_one(session)?;
                }
            }
        }
        Ok(())
    }

    /// One muted frame with the current inputs.
    pub(crate) fn run_one(&mut self, session: &mut CoreSession) -> anyhow::Result<()> {
        let video = self.video_in_checksum;
        session.set_av_enable(AvEnable {
            video,
            audio: false,
        });
        let ran = session.run_frame(&mut self.video, &mut self.audio, &mut self.input);
        session.set_av_enable(AvEnable::ALL);
        ran?;
        let index = self.frame.max(0) as usize;
        if self.advances.len() <= index {
            self.advances.resize(index + 1, 0);
        }
        self.advances[index] = self.advances[index].saturating_add(1);
        self.frame += 1;
        if let Some(crc) = ram_crc(session) {
            self.crc_after.insert(self.frame, crc);
        }
        Ok(())
    }

    fn checksum(&mut self, session: &mut CoreSession) -> u128 {
        let video = match self.video_in_checksum {
            true => u128::from(self.video.last_checksum) << 32,
            false => 0,
        };
        video | self.ram_checksum(session)
    }

    fn ram_checksum(&mut self, session: &mut CoreSession) -> u128 {
        match ram_crc(session) {
            Some(crc) => {
                self.source.get_or_insert(ChecksumSource::SystemRam);
                u128::from(crc)
            }
            None => {
                self.source = Some(ChecksumSource::StateBlob);
                u128::from(crc32fast::hash(&self.buf))
            }
        }
    }

    /// (frames advanced once, frames advanced more than once).
    pub(crate) fn advance_histogram(&self) -> (usize, usize) {
        let once = self.advances.iter().filter(|&&n| n == 1).count();
        let more = self.advances.iter().filter(|&&n| n > 1).count();
        (once, more)
    }
}

/// (p50, p95) of `samples` in ms (0 when empty).
#[cfg(test)]
pub(crate) fn percentiles(samples: &[f64]) -> (f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q).round() as usize];
    (at(0.5), at(0.95))
}
