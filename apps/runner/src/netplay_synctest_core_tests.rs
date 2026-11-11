//! Real-core rollback check promoted from the phase-00 spike: `ggrs::SyncTestSession`
//! (rolls back every frame by `check_distance` and compares re-simulated checksums)
//! driving the runner's own netplay core path (`SessionCore`: rollback context,
//! `NetplayInputSource`, RAM + SRAM checksum). One test per netplay core; `Yes`
//! cores must show 0 mismatches over 3 000 frames, `Experimental` ones are reported.
//! Gated by `NOSTYRA_NETPLAY_SYNCTEST=1` (cores from `cores/`, ROMs from
//! `test-assets/roms/`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use ggrs::{GgrsError, GgrsRequest, SessionBuilder, SyncTestSession};
use input_mapping::NetplayInputSource;
use libretro_host::{AvEnable, NullVideoSink};
use netplay_session::netplay_input::{NetplayInput, NETPLAY_PORTS};
use system_catalog::{NetplaySupport, SystemSpec};

use crate::netplay_runtime::netplay_core::{run_warmup, NetplayCore};
use crate::netplay_runtime::session_core::{SessionCore, SilentHooks};
use crate::real_core_session_loader::{core_path, load_session_in};
use crate::runner_core_options;
use crate::runner_real_core_suite::{repo, scratch, CORE_LOCK};

const FRAMES: u32 = 3_000;

struct SyncConfig;

impl ggrs::Config for SyncConfig {
    type Input = NetplayInput;
    type InputPredictor = ggrs::PredictRepeatLast;
    type State = Vec<u8>;
    type Address = u8;
}

/// Button held 7 frames, a different phase per player (Start / Select skipped).
fn scripted(frame: i32, player: usize) -> NetplayInput {
    const BUTTONS: [u16; 10] = [0, 1, 4, 5, 6, 7, 8, 9, 10, 11];
    let step = (frame.max(0) as usize / 7 + player * 3) % (BUTTONS.len() + 1);
    let buttons = BUTTONS.get(step).map_or(0, |&id| 1 << id);
    NetplayInput {
        buttons,
        axes: [0; 4],
    }
}

/// Mismatch events over `FRAMES` frames (`Err` = a core call failed).
fn run(core: &mut dyn NetplayCore) -> Result<u32, String> {
    let build = || {
        SessionBuilder::<SyncConfig>::new()
            .with_num_players(2)
            .unwrap()
            .with_check_distance(2)
            .start_synctest_session()
            .unwrap()
    };
    let mut sync: SyncTestSession<SyncConfig> = build();
    let (mut mismatches, mut offset) = (0, 0);
    for _ in 0..FRAMES {
        let frame = sync.current_frame();
        for player in 0..2 {
            sync.add_local_input(player, scripted(offset + frame, player))
                .unwrap();
        }
        let requests = match sync.advance_frame() {
            Ok(requests) => requests,
            Err(GgrsError::MismatchedChecksum { .. }) => {
                mismatches += 1;
                offset += frame;
                sync = build();
                continue;
            }
            Err(err) => return Err(err.to_string()),
        };
        for request in requests {
            match request {
                GgrsRequest::SaveGameState { cell, frame } => {
                    let mut buf = Vec::new();
                    core.save_into(&mut buf)?;
                    let checksum = core.ram_checksum();
                    cell.save(frame, Some(buf), Some(checksum));
                }
                GgrsRequest::LoadGameState { cell, .. } => {
                    core.load_from(&cell.load().ok_or("empty cell")?)?;
                }
                GgrsRequest::AdvanceFrame { inputs } => {
                    let mut ports = [NetplayInput::default(); NETPLAY_PORTS];
                    for (port, (input, _)) in ports.iter_mut().zip(inputs) {
                        *port = input;
                    }
                    core.run(&ports, AvEnable::NONE)?;
                }
            }
        }
    }
    Ok(mismatches)
}

fn synctest(core_id: &str, system: &SystemSpec, rom: &str, overrides: &[(&str, &str)]) {
    if std::env::var("NOSTYRA_NETPLAY_SYNCTEST").as_deref() != Ok("1") {
        eprintln!("skipped: set NOSTYRA_NETPLAY_SYNCTEST=1");
        return;
    }
    if !core_path(core_id).exists() {
        eprintln!("SYNCTEST core={core_id} UNTESTED (core missing)");
        return;
    }
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let overrides: Vec<(String, String)> = overrides
        .iter()
        .map(|&(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    let options = runner_core_options::resolve_for_players(system, 2, &overrides);
    let (sys_dir, saves) = (scratch("np-sync-sys"), scratch("np-sync-saves"));
    let rom = repo(&format!("test-assets/roms/{rom}"));
    let mut session = load_session_in(
        core_id,
        &rom,
        (sys_dir.path(), saves.path()),
        &options,
        None,
    );
    let (mut video, mut hooks) = (NullVideoSink::default(), SilentHooks::default());
    let mut input = NetplayInputSource::default();
    let mut core = SessionCore {
        session: &mut session,
        video: &mut video,
        input: &mut input,
        analog: false,
        hooks: &mut hooks,
    };
    let warmup = run_warmup(&mut core).expect("core never serialized");
    let mismatches = run(&mut core).expect("synctest run");
    let support = system_catalog::core_for_system(system.id).map(|c| c.netplay);
    eprintln!(
        "SYNCTEST core={core_id} system={} W={warmup} mismatches={mismatches} support={support:?}",
        system.id
    );
    if support == Some(NetplaySupport::Yes) {
        assert_eq!(mismatches, 0, "{core_id} desyncs under rollback");
    }
}

#[test]
fn synctest_snes9x() {
    synctest("snes9x", &system_catalog::SNES, "nostyra-test.sfc", &[]);
}

#[test]
fn synctest_mesen() {
    synctest("mesen", &system_catalog::NES, "nostyra-test.nes", &[]);
}

#[test]
fn synctest_genesis_plus_gx() {
    use system_catalog::{GENESIS, GG, SMS};
    synctest("genesis_plus_gx", &GENESIS, "nostyra-test.gen", &[]);
    synctest("genesis_plus_gx", &SMS, "nostyra-test.sms", &[]);
    synctest("genesis_plus_gx", &GG, "nostyra-test.gg", &[]);
}

#[test]
fn synctest_stella() {
    synctest(
        "stella",
        &system_catalog::ATARI2600,
        "nostyra-test.a26",
        &[],
    );
}

#[test]
fn synctest_mgba() {
    synctest("mgba", &system_catalog::GB, "nostyra-test.gb", &[]);
    synctest("mgba", &system_catalog::GBA, "nostyra-test.gba", &[]);
}

#[test]
fn synctest_experimental_cores() {
    let hle = [("pcsx_rearmed_bios", "HLE")];
    synctest(
        "pcsx-rearmed",
        &system_catalog::PSX,
        "nostyra-test-psx.cue",
        &hle,
    );
    synctest(
        "prosystem",
        &system_catalog::ATARI7800,
        "nostyra-test.a78",
        &[],
    );
}
