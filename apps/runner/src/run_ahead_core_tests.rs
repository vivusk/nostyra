//! Run-ahead against a real core + test ROM, driven exactly like the runner loop
//! (`RunAhead::run_tick`). Called from `runner_real_core_suite` (snes9x, mesen, mgba).
//!
//! - Determinism (M3 exit criterion): 600 real frames of scripted input (button
//!   changes every 7 frames, a catch-up tick every 50) give the same SRAM at
//!   run-ahead 0, 1 and 2, and — where the core guarantees it
//!   ([`RunAheadExpectations`]) — the same final state and real audio stream.
//! - Presented frame: with input held, run-ahead N shows at tick t what run-ahead 0
//!   shows at tick t+N.

use std::time::{Duration, Instant};
use std::{cell::Cell, rc::Rc};

use libretro_host::{AudioSink, CoreSession, InputSource, NullAudioSink, NullVideoSink};
use system_catalog::SaveStateQuality::Deterministic;

use crate::emulation_main_loop::run_ahead::{RealFrameHooks, RunAhead};

const REAL_FRAMES: u32 = 600;

/// What a core guarantees under run-ahead (mGBA: same relaxations, see
/// `handheld_real_core_suite_tests`). Mesen (M6 phase-00 spike): its serialized
/// state keeps one byte that differs after an unserialize, and it drops its pending
/// audio (about half a frame) once on the first unserialize — gameplay, SRAM and the
/// presented frames are still exact, so only those two checks relax.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunAheadExpectations {
    pub exact_state: bool,
    pub exact_audio: bool,
    /// A state the core was restored from serializes back to the same bytes (rewind
    /// check). False for mGBA in GBA mode: it recomputes 4 bytes on its first load.
    pub exact_round_trip: bool,
    /// Bytes two serializes of the same moment may differ in (rewind checks compare
    /// with this tolerance). 0 for every core but PCSX ReARMed, whose serialize
    /// bumps a counter inside the state on each call (M8 phase 05).
    pub state_jitter_bytes: usize,
}

/// Every check strict (snes9x).
pub(crate) const EXACT_RUN_AHEAD: RunAheadExpectations = RunAheadExpectations {
    exact_state: true,
    exact_audio: true,
    exact_round_trip: true,
    state_jitter_bytes: 0,
};

/// Input indexed by the real-timeline frame number (shared with the hooks), so
/// hidden run-ahead frames see the input of the real frame they shadow.
struct ScriptedInput {
    real_frame: Rc<Cell<u32>>,
    /// Some(id): that button held for the whole run instead of the script.
    held: Option<u32>,
}

impl InputSource for ScriptedInput {
    fn poll(&mut self) {}

    fn state(&mut self, port: u32, _device: u32, _index: u32, id: u32) -> i16 {
        // 12 joypad buttons in turn, 7 frames each, then 7 frames with nothing held.
        let scripted = (self.real_frame.get() / 7) % 13;
        let pressed = self.held.unwrap_or(scripted);
        i16::from(port == 0 && id == pressed)
    }
}

/// Real-timeline hooks: counts real frames and hashes the real audio stream.
#[derive(Default)]
struct TestHooks {
    real_frame: Rc<Cell<u32>>,
    audio: HashingAudioSink,
    muted: NullAudioSink,
}

#[derive(Default)]
struct HashingAudioSink {
    hash: u64,
    samples: u64,
}

impl AudioSink for HashingAudioSink {
    fn samples(&mut self, interleaved: &[i16]) {
        for &s in interleaved {
            self.hash = (self.hash ^ u64::from(s as u16)).wrapping_mul(0x0100_0000_01b3);
        }
        self.samples += interleaved.len() as u64;
    }
}

impl RealFrameHooks for TestHooks {
    fn before_real_frame(&mut self) {}

    fn audio_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        &mut self.audio
    }

    fn muted_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        &mut self.muted
    }

    fn after_real_frame(&mut self, _session: &mut CoreSession, _state: Option<&[u8]>) {
        self.real_frame.set(self.real_frame.get() + 1);
    }
}

/// Outcome of one run from the frame-0 state.
struct RunResult {
    state: Vec<u8>,
    sram: Vec<u8>,
    audio: (u64, u64),
    /// Checksum of the frame presented after each tick.
    presented: Vec<u64>,
    emulate_time: Duration,
    ticks: u32,
}

/// Runs `real_frames` real frames at `run_ahead` from `frame0`. `catch_up`: every
/// 50th tick has two frames due.
fn run(
    session: &mut CoreSession,
    frame0: &[u8],
    run_ahead: u8,
    real_frames: u32,
    held: Option<u32>,
    catch_up: bool,
) -> RunResult {
    session.unserialize(frame0).unwrap();
    let mut ra = RunAhead::new(run_ahead, session, Deterministic);
    let mut hooks = TestHooks::default();
    let mut input = ScriptedInput {
        real_frame: hooks.real_frame.clone(),
        held,
    };
    let mut video = NullVideoSink::default();
    let (mut presented, mut emulate_time, mut ticks) = (Vec::new(), Duration::ZERO, 0);
    while hooks.real_frame.get() < real_frames {
        let wanted = if catch_up && ticks % 50 == 49 { 2 } else { 1 };
        let due = wanted.min(real_frames - hooks.real_frame.get());
        let started = Instant::now();
        ra.run_tick(due, session, &mut video, &mut input, &mut hooks)
            .unwrap();
        emulate_time += started.elapsed();
        ticks += 1;
        presented.push(video.last_checksum);
    }
    assert_eq!(ra.active_frames(), run_ahead, "run-ahead stayed on");
    RunResult {
        state: session.serialize().unwrap(),
        // Empty for content without battery RAM (the SG-1000 test cart).
        sram: session
            .save_ram_mut()
            .map(|ram| ram.to_vec())
            .unwrap_or_default(),
        audio: (hooks.audio.hash, hooks.audio.samples),
        presented,
        emulate_time,
        ticks,
    }
}

pub(crate) fn check_run_ahead(session: &mut CoreSession, expect: RunAheadExpectations) {
    let frame0 = session.serialize().unwrap();

    let off = run(session, &frame0, 0, REAL_FRAMES, None, true);
    assert!(off.audio.1 > 0, "real frames produce audio");
    for frames in [1, 2] {
        let on = run(session, &frame0, frames, REAL_FRAMES, None, true);
        assert!(
            !expect.exact_state || on.state == off.state,
            "run-ahead {frames}: final state differs"
        );
        assert_eq!(on.sram, off.sram, "run-ahead {frames}: SRAM differs");
        if expect.exact_audio {
            assert_eq!(
                on.audio, off.audio,
                "run-ahead {frames}: audio stream differs"
            );
        }
        let per_tick = on.emulate_time / on.ticks;
        eprintln!("run-ahead {frames}: avg emulate time per tick {per_tick:?}");
    }
    eprintln!(
        "run-ahead 0: avg emulate time per tick {:?}",
        off.emulate_time / off.ticks
    );

    // Presented frame: input held (B = id 0), no catch-up ticks.
    let held = Some(0);
    let plain = run(session, &frame0, 0, 62, held, false).presented;
    assert!(plain.windows(2).all(|w| w[0] != w[1]), "frames must differ");
    for frames in [1u8, 2] {
        let ahead = run(session, &frame0, frames, 60, held, false).presented;
        let n = usize::from(frames);
        assert_eq!(
            ahead[..],
            plain[n..60 + n],
            "run-ahead {frames} shows tick t+{n}"
        );
    }
}
