//! `netplay_launch::prepare`: CLI / env checks, registry eligibility and the room
//! size every role applies before the load (F21).

use std::path::Path;

use clap::Parser;
use netplay_session::room_api_types::IceServer;
use system_catalog::{GENESIS, PSX, SNES};

use super::*;
use crate::runner_core_options::{port_devices_for, resolve_for_players};

fn env(role: SeatRole, max_players: u8) -> String {
    NetplaySessionEnv {
        server_ws_url: "ws://127.0.0.1:3536/v1/ws".into(),
        room_id: "abcd1234".into(),
        code: None,
        token: "tok.sig".into(),
        role,
        max_players,
        ice_servers: vec![IceServer {
            urls: vec!["stun:127.0.0.1:3478".into()],
            username: None,
            credential: None,
        }],
        expires_at: 1,
    }
    .to_json()
    .unwrap()
}

fn args(extra: &[&str]) -> RunnerCliArgs {
    let base = ["nostyra-runner", "--core", "c", "--rom", "r"];
    RunnerCliArgs::try_parse_from(base.iter().chain(extra)).unwrap()
}

fn code_of(err: &anyhow::Error) -> u8 {
    crate::runner_exit_codes::exit_code_for(err)
}

#[test]
fn guest_applies_the_room_size_before_load() {
    let reporter = SessionReporter::new(false, Path::new("r"));
    let join = args(&["--netplay", "join"]);
    let launch = prepare(&join, &SNES, Some(env(SeatRole::Player, 4)), &reporter)
        .unwrap()
        .expect("netplay launch");
    assert_eq!(players(Some(&launch)), 4);
    assert_eq!(
        port_devices_for(&SNES, players(Some(&launch))),
        [1, 257],
        "multitap on port 2"
    );
    assert_eq!(
        port_devices_for(&GENESIS, 4)[0],
        1537,
        "Team Player on port A"
    );
    let psx = prepare(&join, &PSX, Some(env(SeatRole::Player, 4)), &reporter).unwrap();
    let options = resolve_for_players(&PSX, players(psx.as_ref()), &[]);
    let multitap = ("pcsx_rearmed_multitap".to_owned(), "port 1".to_owned());
    assert!(options.contains(&multitap), "multitap option sent at load");
    let two = prepare(&join, &SNES, Some(env(SeatRole::Player, 2)), &reporter).unwrap();
    assert!(
        port_devices_for(&SNES, players(two.as_ref())).is_empty(),
        "defaults"
    );
    assert_eq!(players(None), LOCAL_PLAYERS);
}

#[test]
fn usage_and_eligibility_refusals() {
    let reporter = SessionReporter::new(false, Path::new("r"));
    let host3 = args(&["--netplay", "host", "--netplay-players", "3"]);
    let err = prepare(&host3, &SNES, Some(env(SeatRole::Host, 4)), &reporter)
        .err()
        .unwrap();
    assert_eq!(code_of(&err), 2, "--netplay-players differs from the room");
    let join = args(&["--netplay", "join"]);
    for bad in [None, Some("{".to_owned()), Some(env(SeatRole::Host, 2))] {
        let err = prepare(&join, &SNES, bad, &reporter).err().unwrap();
        assert_eq!(code_of(&err), 2, "missing / malformed env or wrong seat");
    }
    let n64 = system_catalog::system("n64").unwrap();
    let err = prepare(&join, n64, Some(env(SeatRole::Player, 2)), &reporter)
        .err()
        .unwrap();
    assert_eq!(
        i32::from(code_of(&err)),
        ipc_protocol::EXIT_NETPLAY_INELIGIBLE,
        "GL core"
    );
    let local = prepare(&args(&[]), &SNES, Some(env(SeatRole::Player, 2)), &reporter);
    assert!(local.unwrap().is_none(), "no --netplay: local play");
}
