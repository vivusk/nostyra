//! Phase-00 T2: `ggrs::SyncTestSession` per core (single instance, rolls back every
//! frame by `check_distance` and compares checksums of re-simulated frames). Gated by
//! `NOSTYRA_NETPLAY_SYNCTEST=1` (optional `NOSTYRA_NETPLAY_SYNCTEST_CORES=a,b` filter);
//! prints one `SYNCTEST` line per content for the spike report.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use ggrs::{GgrsError, SessionBuilder, SyncTestSession};
use libretro_host::{CoreSession, SavestateContext};
use system_catalog::SystemSpec;

use super::core_rollback_game::{percentiles, CoreRollbackGame};
use super::spike_config::{scripted_input, SpikeConfig};
use crate::real_core_session_loader::{core_path, load_session_in};
use crate::runner_core_options;
use crate::runner_real_core_suite::{repo, scratch, CORE_LOCK};

type Session = SyncTestSession<SpikeConfig<u8>>;

const FRAMES: u32 = 3_000;
const LONG_DISTANCE_FRAMES: u32 = 600;
const MAX_WARMUP: u32 = 600;

/// (core, system, test ROM, option overrides).
type Case = (
    &'static str,
    &'static SystemSpec,
    &'static str,
    Vec<(String, String)>,
);

fn cases() -> Vec<Case> {
    use system_catalog::*;
    let hle = vec![("pcsx_rearmed_bios".to_owned(), "HLE".to_owned())];
    let nds_time = [("mode", "absolute"), ("absolute_year", "2004")]
        .map(|(k, v)| (format!("melonds_start_time_{k}"), v.to_owned()))
        .to_vec();
    vec![
        ("snes9x", &SNES, "nostyra-test.sfc", vec![]),
        ("mesen", &NES, "nostyra-test.nes", vec![]),
        ("genesis_plus_gx", &GENESIS, "nostyra-test.gen", vec![]),
        ("genesis_plus_gx", &SMS, "nostyra-test.sms", vec![]),
        ("genesis_plus_gx", &GG, "nostyra-test.gg", vec![]),
        ("stella", &ATARI2600, "nostyra-test.a26", vec![]),
        ("mgba", &GB, "nostyra-test.gb", vec![]),
        ("mgba", &GBA, "nostyra-test.gba", vec![]),
        ("pcsx-rearmed", &PSX, "nostyra-test-psx.cue", hle),
        ("prosystem", &ATARI7800, "nostyra-test.a78", vec![]),
        ("handy", &LYNX, "nostyra-test.lnx", vec![]),
        ("melonds_ds", &NDS, "nostyra-test.nds", nds_time),
    ]
}

fn build(check_distance: usize) -> Session {
    SessionBuilder::<SpikeConfig<u8>>::new()
        .with_num_players(2)
        .unwrap()
        .with_check_distance(check_distance)
        .start_synctest_session()
        .unwrap()
}

/// Frames run with no input until a `MUST_INITIALIZE` core serializes (`W`).
fn warm_up(session: &mut CoreSession, game: &mut CoreRollbackGame) -> u32 {
    let mut buf = Vec::new();
    for w in 0..MAX_WARMUP {
        if session
            .serialize_into(&mut buf, SavestateContext::RunaheadSameInstance)
            .is_ok()
        {
            game.frame = 0;
            return w;
        }
        game.run_one(session).unwrap();
    }
    panic!("no state after {MAX_WARMUP} warm-up frames");
}

/// Runs `frames` synctest frames; (mismatch events, first mismatched frame, error).
fn run(
    session: &mut CoreSession,
    game: &mut CoreRollbackGame,
    distance: usize,
    frames: u32,
) -> (u32, Option<i32>, Option<String>) {
    let (mut sync, mut events, mut first, mut offset) = (build(distance), 0, None, 0);
    game.frame = 0;
    for _ in 0..frames {
        let frame = sync.current_frame();
        for player in 0..2 {
            let input = scripted_input(offset + frame, player);
            sync.add_local_input(player, input).unwrap();
        }
        match sync.advance_frame() {
            Ok(requests) => {
                if let Err(error) = game.handle(session, requests) {
                    return (events, first, Some(format!("{error:#}")));
                }
            }
            Err(GgrsError::MismatchedChecksum {
                mismatched_frames, ..
            }) => {
                events += 1;
                first.get_or_insert(offset + mismatched_frames[0]);
                offset += frame; // restart from where the core is
                sync = build(distance);
                game.frame = 0;
            }
            Err(error) => return (events, first, Some(error.to_string())),
        }
    }
    (events, first, None)
}

#[test]
fn synctest_per_core() {
    if std::env::var("NOSTYRA_NETPLAY_SYNCTEST").as_deref() != Ok("1") {
        eprintln!("skipped: set NOSTYRA_NETPLAY_SYNCTEST=1");
        return;
    }
    let only = std::env::var("NOSTYRA_NETPLAY_SYNCTEST_CORES").unwrap_or_default();
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    for (core, system, rom, overrides) in cases() {
        if !only.is_empty() && !only.split(',').any(|c| c == core) {
            continue;
        }
        if !core_path(core).exists() {
            eprintln!("SYNCTEST core={core} rom={rom} verdict=UNTESTED (core missing)");
            continue;
        }
        let (sys_dir, saves) = (scratch("synctest-sys"), scratch("synctest-saves"));
        let options = runner_core_options::resolve(system, &overrides);
        let rom_path = repo(&format!("test-assets/roms/{rom}"));
        let mut session = load_session_in(
            core,
            &rom_path,
            (sys_dir.path(), saves.path()),
            &options,
            None,
        );
        let video_in_checksum =
            std::env::var("NOSTYRA_NETPLAY_SYNCTEST_VIDEO").as_deref() == Ok("1");
        let mut game = CoreRollbackGame::default();
        game.video_in_checksum = video_in_checksum;
        let warmup = warm_up(&mut session, &mut game);
        let state_len = session.current_state_size();
        let (events, first, error) = run(&mut session, &mut game, 2, FRAMES);
        let (ser, unser) = (
            percentiles(&game.serialize_ms),
            percentiles(&game.unserialize_ms),
        );
        let (events7, first7, error7) = match error {
            None => run(&mut session, &mut game, 7, LONG_DISTANCE_FRAMES),
            Some(_) => (0, None, None),
        };
        let verdict = match (&error, events + events7) {
            (Some(_), _) => "FAIL",
            (None, 0) => "PASS",
            _ => "FLAKY",
        };
        eprintln!(
            "SYNCTEST video={video_in_checksum} core={core} rom={rom} verdict={verdict} W={warmup} state={state_len} \
             d2_mismatch={events} first={first:?} d7_mismatch={events7} first7={first7:?} \
             ser_ms_p50={:.3} p95={:.3} unser_ms_p50={:.3} p95={:.3} loads={} source={:?} \
             error={error:?} error7={error7:?}",
            ser.0, ser.1, unser.0, unser.1, game.loads, game.source
        );
    }
}
