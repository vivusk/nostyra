#![allow(clippy::unwrap_used)]

use super::harness::{cfg, Room, HOST};
use super::*;
use crate::session_fingerprint::tests::sample as fingerprint;
use crate::session_fingerprint::SessionFingerprint;

#[test]
fn host_flow_two_players_sync_then_start() {
    let (a, b) = (PeerKey(7), PeerKey(3));
    let fp4 = SessionFingerprint {
        max_players: 4,
        ..fingerprint()
    };
    let mut room = Room::new(fp4.clone());
    room.add(a, cfg(SeatRole::Player, fp4.clone()));
    assert!(room.node(HOST).1.can_start(), "one guest greeted");
    room.add(b, cfg(SeatRole::Player, fp4));
    assert_eq!(room.phase(a), LobbyPhase::InLobby);
    assert_eq!(room.node(b).1.host(), Some(HOST));
    let order: Vec<_> = room
        .node(HOST)
        .1
        .roster()
        .iter()
        .map(|m| (m.peer, m.handle))
        .collect();
    assert_eq!(
        order,
        vec![(HOST, Some(0)), (a, Some(1)), (b, Some(2))],
        "join order"
    );
    assert_eq!(
        room.node(a).1.roster().len(),
        3,
        "RosterUpdate reached the first guest"
    );
    room.start();
    for key in [HOST, a, b] {
        assert_eq!(room.phase(key), LobbyPhase::Running, "{key:?}");
        let plans = room.plans(key);
        let [LobbyEffect::StartDriver(plan)] = plans.as_slice() else {
            panic!("{plans:?}")
        };
        assert_eq!(plan.handles.players(), &[(HOST, 0), (a, 1), (b, 2)]);
        assert_eq!(plan.input_delay, 1);
        assert_eq!(plan.me, key);
    }
}

#[test]
fn start_waits_for_every_sync_ack() {
    let a = PeerKey(7);
    let mut room = Room::new(fingerprint());
    room.add(a, cfg(SeatRole::Player, fingerprint()));
    room.muted.push(a); // a's SyncAck never arrives
    room.start();
    assert_eq!(room.phase(HOST), LobbyPhase::Syncing);
    assert_eq!(room.phase(a), LobbyPhase::Starting);
    assert!(room.plans(HOST).is_empty());
}

#[test]
fn spectator_joins_and_gets_a_handle_at_start() {
    let (a, s) = (PeerKey(7), PeerKey(1));
    let mut room = Room::new(fingerprint());
    room.add(s, cfg(SeatRole::Spectator, fingerprint()));
    room.add(a, cfg(SeatRole::Player, fingerprint()));
    assert_eq!(room.node(s).1.my_handle(), None);
    room.start();
    let plans = room.plans(s);
    let [LobbyEffect::StartDriver(plan)] = plans.as_slice() else {
        panic!("{plans:?}")
    };
    assert_eq!(plan.role, SeatRole::Spectator);
    assert_eq!(plan.local_handle(), None);
    assert_eq!(plan.handles.spectators(), &[(s, 2)]);
    assert_eq!(room.node(s).1.my_handle(), Some(2));
}

#[test]
fn host_leaving_ends_the_guests() {
    let a = PeerKey(7);
    let mut room = Room::new(fingerprint());
    room.add(a, cfg(SeatRole::Player, fingerprint()));
    room.leave(HOST);
    assert_eq!(room.phase(a), LobbyPhase::Ended(EndReason::HostLeft));
}

#[test]
fn guest_runs_the_warmup_before_loading_the_sync() {
    let fp = SessionFingerprint {
        warmup_frames: 3,
        ..fingerprint()
    };
    let a = PeerKey(7);
    let mut room = Room::new(fp.clone());
    room.add(a, cfg(SeatRole::Player, fp));
    room.take(a);
    room.start();
    let effects = room.take(a);
    let warmup = effects
        .iter()
        .position(|e| *e == LobbyEffect::RunWarmup(3))
        .unwrap();
    let load = effects
        .iter()
        .position(|e| matches!(e, LobbyEffect::LoadSync(_)))
        .unwrap();
    assert!(warmup < load);
}

#[test]
fn lobby_handles_are_renumbered_when_a_player_leaves_before_start() {
    let (a, b, c) = (PeerKey(7), PeerKey(3), PeerKey(9));
    let mut fp = fingerprint();
    fp.max_players = 4;
    let mut room = Room::new(fp.clone());
    room.add(a, cfg(SeatRole::Player, fp.clone()));
    room.add(b, cfg(SeatRole::Player, fp.clone()));
    room.leave(a);
    room.add(c, cfg(SeatRole::Player, fp));
    let handles: Vec<_> = room
        .node(HOST)
        .1
        .roster()
        .iter()
        .map(|m| m.handle)
        .collect();
    assert_eq!(handles, [Some(0), Some(1), Some(2)]);
    assert_eq!(room.node(b).1.my_handle(), Some(1));
    assert_eq!(room.node(c).1.my_handle(), Some(2));
}

#[test]
fn guest_error_exit_sends_leave_to_the_host() {
    let mut guest = LobbyState::new(cfg(SeatRole::Player, fingerprint()));
    let now = std::time::Instant::now();
    guest.apply(LobbyEvent::PeerJoined(HOST), now);
    guest.apply(LobbyEvent::Connected { me: PeerKey(7) }, now);
    let welcome = crate::lobby_protocol::LobbyMessage::Welcome {
        roster: Vec::new(),
        your_handle: Some(1),
    };
    guest.apply(LobbyEvent::Message(HOST, welcome), now);
    let begin = crate::lobby_protocol::LobbyMessage::SyncBegin {
        sram_len: 4,
        rtc_len: None,
        state_len: 8,
        warmup_frames: fingerprint().warmup_frames,
    };
    guest.apply(LobbyEvent::Message(HOST, begin), now);
    assert_eq!(guest.phase(), &LobbyPhase::Syncing);
    let chunk = crate::lobby_protocol::LobbyMessage::SyncChunk {
        kind: crate::lobby_protocol::SyncKind::State,
        offset: 1_000,
        bytes: vec![1],
    };
    let effects = guest.apply(LobbyEvent::Message(HOST, chunk), now);
    assert!(matches!(
        guest.phase(),
        LobbyPhase::Ended(EndReason::Error(_))
    ));
    let leave = LobbyEffect::Send {
        to: vec![HOST],
        msg: crate::lobby_protocol::LobbyMessage::Leave,
    };
    assert!(effects.contains(&leave));
}
