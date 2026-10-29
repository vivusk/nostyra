//! Rewind against a real core (snes9x, mesen, mgba), driven like the runner loop (captures from
//! `RunAhead::run_tick`'s real-frame hooks). Called from `runner_real_core_suite`.
//!
//! - `check_rewind`: 300 real frames captured (run-ahead 0 and 1, the latter reusing
//!   run-ahead's serialize); 50 pops give back the states of those frames
//!   byte-for-byte, and the core restored from one serializes to the same bytes
//!   (`exact_round_trip`; else, as mGBA in GBA mode recomputes a few bytes on load,
//!   the restored core's state round-trips unchanged from then on). "Same" allows
//!   `state_jitter_bytes` differing bytes (PCSX ReARMed bumps a counter per serialize).
//! - `measure_history`: 60 s of play, average entry size and seconds of history in
//!   the 64 MiB default budget at granularity 2–4 (ignored measurement test).

use std::time::{Duration, Instant};

use config_layers::RewindConfig;
use libretro_host::{AudioSink, CoreSession, InputSource, NullAudioSink, NullVideoSink};
use libretro_host::{NullInputSource, SavestateContext};
use system_catalog::SaveStateQuality::Deterministic;

use crate::emulation_main_loop::rewind_playback::rewind_state::REWIND_GRANULARITY;
use crate::emulation_main_loop::rewind_playback::Rewind;
use crate::emulation_main_loop::run_ahead::{RealFrameHooks, RunAhead};
use crate::rewind_ring::RewindRing;
use crate::run_ahead_core_tests::RunAheadExpectations;

const CTX: SavestateContext = SavestateContext::RunaheadSameInstance;
const DEFAULT_BUDGET: usize = 64 << 20;

/// Feeds rewind captures and records, independently, the state of every frame a
/// capture is due on.
struct CaptureHooks {
    rewind: Rewind,
    real_frame: u32,
    expected: Vec<Vec<u8>>,
    reused: u32,
    audio: NullAudioSink,
}

impl RealFrameHooks for CaptureHooks {
    fn before_real_frame(&mut self) {}

    fn audio_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        &mut self.audio
    }

    fn muted_sink(&mut self) -> &mut (dyn AudioSink + 'static) {
        &mut self.audio
    }

    fn after_real_frame(&mut self, session: &mut CoreSession, state: Option<&[u8]>) {
        self.real_frame += 1;
        self.reused += u32::from(state.is_some());
        self.rewind.after_real_frame(session, state, || None);
        if self.real_frame.is_multiple_of(REWIND_GRANULARITY) {
            let mut buf = Vec::new();
            session.serialize_into(&mut buf, CTX).unwrap();
            self.expected.push(buf);
        }
    }
}

pub(crate) fn check_rewind(session: &mut CoreSession, expect: RunAheadExpectations) {
    let same = |a: &[u8], b: &[u8]| {
        a.len() == b.len()
            && a.iter().zip(b).filter(|(x, y)| x != y).count() <= expect.state_jitter_bytes
    };
    let frame0 = session.serialize().unwrap();
    let config = RewindConfig {
        enabled: true,
        buffer_mb: 64,
    };
    for run_ahead in [0, 1] {
        session.unserialize(&frame0).unwrap();
        let mut ra = RunAhead::new(run_ahead, session, Deterministic);
        let mut hooks = CaptureHooks {
            rewind: Rewind::new(&config, session, Deterministic),
            real_frame: 0,
            expected: Vec::new(),
            reused: 0,
            audio: NullAudioSink::default(),
        };
        let (mut video, mut input) = (NullVideoSink::default(), NullInputSource);
        while hooks.real_frame < 300 {
            ra.run_tick(1, session, &mut video, &mut input, &mut hooks)
                .unwrap();
        }
        assert_eq!(hooks.reused > 0, run_ahead > 0, "run-ahead state reused");
        let newest = hooks.expected.len() - 1;
        for back in 1..=50 {
            let state = hooks.rewind.pop_captures(1).unwrap().to_vec();
            assert!(
                same(&state, &hooks.expected[newest - back]),
                "run-ahead {run_ahead}: pop {back} gives the captured state"
            );
            if back == 50 {
                session.unserialize_from(&state, CTX).unwrap();
                let mut restored = Vec::new();
                session.serialize_into(&mut restored, CTX).unwrap();
                if expect.exact_round_trip {
                    assert!(same(&restored, &state), "core restored the rewound state");
                } else {
                    session.unserialize_from(&restored, CTX).unwrap();
                    let mut again = Vec::new();
                    session.serialize_into(&mut again, CTX).unwrap();
                    assert!(same(&again, &restored), "restored state round-trips");
                }
            }
        }
        hooks.rewind.clear();
        assert!(hooks.rewind.pop_captures(1).is_none(), "cleared");
    }
    crate::rewind_hardcore_core_tests::check_hardcore_rewind(session, &config);
}

/// Scripted play: Start tapped every 2 s (menus), otherwise right held with B/Y
/// tapped, so a real game gets past its title screen and moves.
struct PlayInput {
    frame: u32,
}

impl InputSource for PlayInput {
    fn poll(&mut self) {}

    fn state(&mut self, port: u32, _device: u32, _index: u32, id: u32) -> i16 {
        const B: u32 = 0;
        const Y: u32 = 1;
        const START: u32 = 3;
        const RIGHT: u32 = 7;
        let f = self.frame;
        let held = match id {
            START => f % 120 < 4,
            RIGHT => f % 120 >= 30,
            B => f % 45 < 10,
            Y => f % 20 < 12,
            _ => false,
        };
        i16::from(port == 0 && held)
    }
}

/// Prints entry size and seconds of history in the default 64 MiB budget for
/// granularity 2, 3 and 4 over `seconds` of scripted play.
pub(crate) fn measure_history(session: &mut CoreSession, label: &str, seconds: u32) {
    let fps = session.av_info().timing.fps;
    let frames = (f64::from(seconds) * fps) as u32;
    let mut rings = [2u32, 3, 4].map(|g| (g, RewindRing::new(usize::MAX), Duration::ZERO));
    let (mut video, mut audio) = (NullVideoSink::default(), NullAudioSink::default());
    let mut input = PlayInput { frame: 0 };
    let mut buf = Vec::new();
    for frame in 1..=frames {
        input.frame = frame;
        session
            .run_frame(&mut video, &mut audio, &mut input)
            .unwrap();
        session.serialize_into(&mut buf, CTX).unwrap();
        for (granularity, ring, push_time) in &mut rings {
            if frame.is_multiple_of(*granularity) {
                let started = Instant::now();
                ring.push(&buf);
                *push_time += started.elapsed();
            }
        }
    }
    for (granularity, ring, push_time) in &rings {
        let entries = ring.len().max(1);
        let avg = (ring.bytes_used() - buf.len()) as f64 / entries as f64;
        let fit = (DEFAULT_BUDGET - buf.len()) as f64 / avg;
        let history = fit * f64::from(*granularity) / fps;
        eprintln!(
            "{label}: state {} B, granularity {granularity}: {entries} entries, avg entry {:.0} B, \
             push avg {:?} → {history:.0} s of history in 64 MiB",
            buf.len(),
            avg,
            *push_time / entries as u32,
        );
    }
}
