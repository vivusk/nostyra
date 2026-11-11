//! Phase-00 T3: which libretro ports a core queries once a multitap device id /
//! option is set (device ids from the cores' upstream `retro_controller_description`
//! tables, see the spike report). Gated by `NOSTYRA_NETPLAY_SYNCTEST=1`; prints one
//! `MULTITAP` line per case.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::sync::PoisonError;

use system_catalog::SystemSpec;

use super::core_rollback_game::CoreRollbackGame;
use super::spike_config::scripted_input;
use crate::real_core_session_loader::{core_path, load_session_in};
use crate::runner_core_options;
use crate::runner_real_core_suite::{repo, scratch, CORE_LOCK};

/// `RETRO_DEVICE_SUBCLASS(RETRO_DEVICE_JOYPAD, n)`.
const fn pad_subclass(n: u32) -> u32 {
    ((n + 1) << 8) | 1
}

struct Case {
    label: &'static str,
    core: &'static str,
    system: &'static SystemSpec,
    rom: &'static str,
    devices: Vec<u32>,
    options: Vec<(String, String)>,
}

fn case(
    label: &'static str,
    core: &'static str,
    system: &'static SystemSpec,
    rom: &'static str,
) -> Case {
    Case {
        label,
        core,
        system,
        rom,
        devices: vec![],
        options: vec![],
    }
}

fn cases() -> Vec<Case> {
    use system_catalog::*;
    let opt = |k: &str, v: &str| vec![(k.to_owned(), v.to_owned())];
    let hle = opt("pcsx_rearmed_bios", "HLE");
    let mut tap = opt("pcsx_rearmed_multitap", "port 1");
    tap.extend(hle.clone());
    vec![
        case("snes9x default", "snes9x", &SNES, "nostyra-test.sfc"),
        Case {
            devices: vec![1, pad_subclass(0)],
            ..case(
                "snes9x multitap port 2",
                "snes9x",
                &SNES,
                "nostyra-test.sfc",
            )
        },
        case(
            "gpgx md default",
            "genesis_plus_gx",
            &GENESIS,
            "nostyra-test.gen",
        ),
        Case {
            devices: vec![pad_subclass(5), 1],
            ..case(
                "gpgx md 3B teamplayer port 1",
                "genesis_plus_gx",
                &GENESIS,
                "nostyra-test.gen",
            )
        },
        Case {
            devices: vec![1, pad_subclass(6)],
            ..case(
                "gpgx md 6B teamplayer port 2",
                "genesis_plus_gx",
                &GENESIS,
                "nostyra-test.gen",
            )
        },
        Case {
            devices: vec![pad_subclass(3), pad_subclass(3)],
            ..case(
                "gpgx md 3B 4-wayplay",
                "genesis_plus_gx",
                &GENESIS,
                "nostyra-test.gen",
            )
        },
        Case {
            devices: vec![pad_subclass(7), 1],
            ..case(
                "gpgx sms mastertap port 1",
                "genesis_plus_gx",
                &SMS,
                "nostyra-test.sms",
            )
        },
        case("mesen default", "mesen", &NES, "nostyra-test.nes"),
        Case {
            devices: vec![1, 1, pad_subclass(0), pad_subclass(0)],
            ..case(
                "mesen four score (ports 3-4 = gamepad)",
                "mesen",
                &NES,
                "nostyra-test.nes",
            )
        },
        Case {
            options: hle,
            ..case("pcsx default", "pcsx-rearmed", &PSX, "nostyra-test-psx.cue")
        },
        Case {
            options: tap,
            ..case(
                "pcsx multitap port 1",
                "pcsx-rearmed",
                &PSX,
                "nostyra-test-psx.cue",
            )
        },
        case("mgba gba", "mgba", &GBA, "nostyra-test.gba"),
        case("stella", "stella", &ATARI2600, "nostyra-test.a26"),
    ]
}

#[test]
fn multitap_ports_queried() {
    if std::env::var("NOSTYRA_NETPLAY_SYNCTEST").as_deref() != Ok("1") {
        eprintln!("skipped: set NOSTYRA_NETPLAY_SYNCTEST=1");
        return;
    }
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    for case in cases() {
        if !core_path(case.core).exists() {
            eprintln!("MULTITAP {} UNTESTED (core missing)", case.label);
            continue;
        }
        let (sys_dir, saves) = (scratch("multitap-sys"), scratch("multitap-saves"));
        let options = runner_core_options::resolve(case.system, &case.options);
        let rom = repo(&format!("test-assets/roms/{}", case.rom));
        let mut session = load_session_in(
            case.core,
            &rom,
            (sys_dir.path(), saves.path()),
            &options,
            None,
        );
        session.set_controller_port_devices(&case.devices);
        let mut game = CoreRollbackGame::default();
        let boot = session.serialize().unwrap();
        let mut crc_with = |shift: i32, players: usize| {
            session.unserialize(&boot).unwrap();
            for frame in 0..600 {
                game.input
                    .set_inputs((0..players).map(|p| scripted_input(frame - shift, p)));
                game.run_one(&mut session).unwrap();
            }
            super::core_rollback_game::ram_crc(&mut session)
        };
        // Does RAM at frame 600 see the input? (none / script / script 2 frames late / P2 off)
        let crcs = [
            crc_with(0, 0),
            crc_with(0, 4),
            crc_with(2, 4),
            crc_with(0, 1),
        ];
        let ports: Vec<u32> = (0..8)
            .filter(|p| game.input.queried_ports >> p & 1 == 1)
            .collect();
        eprintln!(
            "MULTITAP {} devices={:?} options={:?} queried_ports={ports:?} ram_crc600[none,all,late2,p1only]={crcs:08x?}",
            case.label, case.devices, case.options
        );
    }
}
