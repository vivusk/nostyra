//! Host + player + spectator `NetplayRuntime`s on an in-memory mesh with 3-tick
//! latency and the fake core: lobby → sync → ggrs → frames limit (F24 / F34), the
//! `MUST_INITIALIZE` alignment (F1) and the warm-up refusal.

use std::time::{Duration, Instant};

use input_mapping::PortState;
use netplay_session::lobby_state::{EndReason, LobbyPhase};
use netplay_session::room_api_types::SeatRole;
use netplay_session::session_fingerprint::{SessionFingerprint, PLATFORM};

use super::fake_netplay_core::FakeNetplayCore;
use super::local_mesh::LocalMesh;
use super::netplay_core::{run_warmup, NetplayCore};
use super::{NetplayRuntime, RuntimeOptions};

const FRAME: Duration = Duration::from_micros(16_667);

fn fingerprint(warmup_frames: u32) -> SessionFingerprint {
    SessionFingerprint {
        core_id: "fake".into(),
        core_version: "1".into(),
        core_sha256: "00".into(),
        content_sha1: "11".into(),
        content_crc32: Some(7),
        patch_crc32: None,
        bios_md5s: Vec::new(),
        options_hash: 3,
        system: "snes".into(),
        fps_milli: 60_000,
        platform: PLATFORM.into(),
        analog: false,
        warmup_frames,
        max_players: 2,
    }
}

struct Peer {
    runtime: NetplayRuntime,
    core: FakeNetplayCore,
    local: PortState,
    line: Option<String>,
}

fn peer(mesh: &LocalMesh, id: u128, role: SeatRole, warmup: u32, buttons: u16) -> Peer {
    let mut core = FakeNetplayCore::new(warmup);
    let warmup_frames = run_warmup(&mut core).expect("warm-up");
    let options = RuntimeOptions {
        role,
        nickname: format!("peer{id}"),
        fingerprint: fingerprint(warmup_frames),
        sram_len: core.memory_sizes().0,
        rtc_len: None,
        input_delay: Some(1),
        autostart: (role == SeatRole::Host).then_some(1),
        frames_limit: Some(120),
        experimental: false,
        room_code: None,
    };
    let local = PortState {
        buttons,
        ..PortState::default()
    };
    let runtime = NetplayRuntime::new(mesh.join(id), options, Instant::now());
    Peer {
        runtime,
        core,
        local,
        line: None,
    }
}

/// Runs the room until every peer printed or `max_ticks` passed.
fn run_room(peers: &mut [Peer], mesh: &LocalMesh, max_ticks: u32) {
    let mut now = Instant::now();
    for _ in 0..max_ticks {
        mesh.advance();
        now += FRAME;
        for p in peers.iter_mut() {
            p.runtime.poll_lobby(&mut p.core, now);
            p.runtime.run_due_frames(&mut p.core, p.local, now);
            if let Some(line) = p.runtime.take_print_line() {
                p.line = Some(line);
            }
        }
        if peers.iter().all(|p| p.line.is_some()) {
            return;
        }
    }
}

fn room(buttons: [u16; 2], warmups: [u32; 3]) -> (LocalMesh, Vec<Peer>) {
    let mesh = LocalMesh::new(3);
    let peers = vec![
        peer(&mesh, 1, SeatRole::Host, warmups[0], buttons[0]),
        peer(&mesh, 2, SeatRole::Player, warmups[1], buttons[1]),
        peer(&mesh, 3, SeatRole::Spectator, warmups[2], 0),
    ];
    (mesh, peers)
}

#[test]
fn host_player_spectator_agree_on_confirmed_frame_crc() {
    let (mesh, mut peers) = room([1, 1 << 13], [0, 0, 0]);
    run_room(&mut peers, &mesh, 3_000);
    let lines: Vec<_> = peers.iter().map(|p| p.line.clone()).collect();
    assert!(lines.iter().all(Option::is_some), "{lines:?}");
    assert!(lines.windows(2).all(|w| w[0] == w[1]), "{lines:?}");
    assert!(lines[0].as_deref().unwrap_or("").ends_with("frames=120"));
    let started = |p: &Peer| {
        p.runtime
            .exit_due(Instant::now() + FRAME * 10_000)
            .is_some()
    };
    assert!(peers.iter().all(started), "every peer asks to exit 0");
    // The player's input crossed: idle inputs give another crc at frame 120.
    let (mesh, mut idle) = room([0, 0], [0, 0, 0]);
    run_room(&mut idle, &mesh, 3_000);
    assert_ne!(idle[0].line, lines[0], "remote input changed RAM");
    assert_eq!(idle[0].line, idle[2].line);
}

#[test]
fn must_initialize_cores_start_from_the_same_state() {
    let (mesh, mut peers) = room([1, 2], [3, 3, 3]);
    run_room(&mut peers, &mesh, 3_000);
    let lines: Vec<_> = peers.iter().map(|p| p.line.clone()).collect();
    assert!(lines.iter().all(Option::is_some), "{lines:?}");
    assert!(lines.windows(2).all(|w| w[0] == w[1]), "{lines:?}");
}

#[test]
fn different_warmup_count_is_refused() {
    let (mesh, mut peers) = room([0, 0], [0, 2, 0]);
    peers.truncate(2);
    run_room(&mut peers, &mesh, 200);
    let phase = peers[1].runtime.phase().clone();
    assert!(
        matches!(phase, LobbyPhase::Ended(EndReason::Refused(_))),
        "{phase:?}"
    );
    assert_ne!(peers[0].runtime.phase(), &LobbyPhase::Running);
}

#[test]
fn host_leaving_ends_the_guest_with_host_left() {
    let (mesh, mut peers) = room([1, 2], [0, 0, 0]);
    for p in peers.iter_mut() {
        p.runtime.frames_limit = None; // play on
    }
    run_room(&mut peers, &mesh, 120);
    assert_eq!(peers[1].runtime.phase(), &LobbyPhase::Running);
    mesh.leave(1);
    peers.remove(0);
    run_room(&mut peers, &mesh, 5);
    let guest = &peers[0].runtime;
    assert_eq!(guest.phase(), &LobbyPhase::Ended(EndReason::HostLeft));
    assert!(
        guest.exit_due(Instant::now()).is_none(),
        "end screen for 3 s"
    );
    let exit = guest
        .exit_due(Instant::now() + Duration::from_secs(4))
        .flatten();
    let code = exit.map(|e| i32::from(e.code));
    assert_eq!(code, Some(ipc_protocol::EXIT_NETPLAY_HOST_LEFT));
}

#[test]
fn local_quit_reports_ended_quit_once() {
    let (mesh, mut peers) = room([0, 0], [0, 0, 0]);
    run_room(&mut peers, &mesh, 20);
    let host = &mut peers[0].runtime;
    let _ = host.take_events();
    host.quit(Instant::now());
    host.quit(Instant::now()); // window close after menu Quit
    let quit = ipc_protocol::NetplayEvent::Ended {
        reason: ipc_protocol::NetplayEndReason::Quit,
    };
    assert_eq!(host.take_events(), vec![quit]);
}
