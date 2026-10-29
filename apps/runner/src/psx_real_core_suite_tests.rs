//! PlayStation on PCSX ReARMed (`scripts/fetch-pcsx-rearmed-core.*`) with the
//! self-written test discs, HLE BIOS forced (no BIOS file in CI): frames, Cross and
//! L2 input, audio, run-ahead / slots / rewind, reset, and disk control on the
//! two-disc `.m3u`. One session at a time under `CORE_LOCK`; skipped, with a note,
//! when the core is missing.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use libretro_host::{CoreSession, InputSource, NullAudioSink, NullVideoSink};
use system_catalog::PSX;

use crate::run_ahead_core_tests::RunAheadExpectations;
use crate::runner_core_options;
use crate::runner_real_core_suite::{
    check_frames_and_input, core_path, load_session_with, repo, run_state_checks, scratch,
    CORE_LOCK,
};

const CORE: &str = "pcsx-rearmed";

/// PCSX ReARMed, measured on the test disc (and the phase-00 spike on real discs):
/// every serialize bumps a counter inside the state (two serializes of one moment
/// differ in 1–3 bytes, a load + save in up to 8), so states compare with a small
/// tolerance; the audio stream and presented frames stay exact.
const PCSX_EXPECTATIONS: RunAheadExpectations = RunAheadExpectations {
    exact_state: false,
    exact_audio: true,
    exact_round_trip: true,
    state_jitter_bytes: 16,
};

/// Registry fixed options + the HLE BIOS, as the process tests pass `--core-option`.
fn options() -> Vec<(String, String)> {
    runner_core_options::resolve(&PSX, &[("pcsx_rearmed_bios".into(), "HLE".into())])
}

fn session(file: &str, dir: &std::path::Path) -> CoreSession {
    load_session_with(
        CORE,
        &repo(&format!("test-assets/roms/{file}")),
        dir,
        &options(),
    )
}

#[test]
fn runner_real_core_suite_pcsx_rearmed() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    if !core_path(CORE).exists() {
        eprintln!("skipped: run scripts/fetch-pcsx-rearmed-core first");
        return;
    }
    {
        let temp = scratch("core-suite-pcsx");
        let mut session = session("nostyra-test-psx.cue", temp.path());
        assert_eq!(session.save_ram_mut().map(|card| card.len()), Some(131_072));
        check_frames_and_input(&mut session);
        check_l2_changes_the_frame(&mut session);
        run_state_checks(&mut session, temp.path(), PCSX_EXPECTATIONS);
        check_reset_restarts_the_disc(&mut session);
        assert!(
            temp.path().join("pcsx-card2.mcd").is_file(),
            "card 2 in the save dir"
        );
    }
    let temp = scratch("core-suite-pcsx-m3u");
    let mut session = session("nostyra-test-psx.m3u", temp.path());
    check_disk_control(&mut session);
}

/// Held RetroPad L2 (id 12) changes the picture (the disc turns blue on L2).
fn check_l2_changes_the_frame(session: &mut CoreSession) {
    let start = session.serialize().unwrap();
    let mut frames = |mask: u16| {
        session.unserialize(&start).unwrap();
        let mut video = NullVideoSink::default();
        (0..10)
            .map(|_| {
                let mut input = Held(mask);
                session
                    .run_frame(&mut video, &mut NullAudioSink::default(), &mut input)
                    .unwrap();
                video.last_checksum
            })
            .collect::<Vec<_>>()
    };
    let plain = frames(0);
    assert_ne!(plain, frames(1 << 12), "L2 changes the frame");
    assert_ne!(frames(1 << 12), frames(1 << 0), "L2 differs from Cross");
    session.unserialize(&start).unwrap();
}

/// `retro_reset` boots the disc again: the frame counter (red ramp) restarts.
fn check_reset_restarts_the_disc(session: &mut CoreSession) {
    let mut video = NullVideoSink::default();
    let mut step = |session: &mut CoreSession, n: usize| {
        (0..n)
            .map(|_| {
                session
                    .run_frame(&mut video, &mut NullAudioSink::default(), &mut Held(0))
                    .unwrap();
                video.last_checksum
            })
            .collect::<Vec<_>>()
    };
    let card = session.save_ram_mut().unwrap().to_vec();
    let before = step(session, 60);
    session.reset();
    let after = step(session, 60);
    assert!(
        after.windows(2).any(|w| w[0] != w[1]),
        "frames change after reset"
    );
    assert_ne!(before, after, "the program restarted");
    assert_eq!(
        session.save_ram_mut().unwrap(),
        card,
        "reset keeps memory card 1"
    );
}

/// Two-disc `.m3u`: count, labels, swap, and states restoring the disc index.
fn check_disk_control(session: &mut CoreSession) {
    let run = |session: &mut CoreSession, n| {
        for _ in 0..n {
            session
                .run_frame(
                    &mut NullVideoSink::default(),
                    &mut NullAudioSink::default(),
                    &mut Held(0),
                )
                .unwrap();
        }
    };
    run(session, 30);
    let (count, labels, index) = {
        let discs = session.discs().expect("disk control registered");
        let labels: Vec<String> = (0..discs.count()).map(|i| discs.label(i)).collect();
        (discs.count(), labels, discs.index())
    };
    assert_eq!((count, index), (2, 0));
    assert_eq!(labels, ["nostyra-test-psx", "nostyra-test-psx-disc2"]);
    let on_disc1 = session.serialize().unwrap();
    session.discs().unwrap().swap_to(1).unwrap();
    run(session, 30);
    assert_eq!(session.discs().unwrap().index(), 1);
    let on_disc2 = session.serialize().unwrap();
    session.unserialize(&on_disc1).unwrap();
    assert_eq!(session.discs().unwrap().index(), 0, "state restores disc 1");
    session.unserialize(&on_disc2).unwrap();
    assert_eq!(session.discs().unwrap().index(), 1, "state restores disc 2");
    assert!(session.discs().unwrap().swap_to(7).is_err(), "no disc 8");
    assert_eq!(
        session.discs().unwrap().index(),
        1,
        "failed swap keeps the disc"
    );
}

/// Port 0 buttons held as a RetroPad bitmask.
struct Held(u16);

impl InputSource for Held {
    fn poll(&mut self) {}

    fn state(&mut self, port: u32, _device: u32, _index: u32, id: u32) -> i16 {
        i16::from(port == 0 && id < 16 && self.0 & (1 << id) != 0)
    }
}
