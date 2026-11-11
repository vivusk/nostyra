#![allow(clippy::unwrap_used)]

use std::time::Duration;

use super::harness::{cfg, Room, HOST};
use super::*;
use crate::lobby_protocol::LobbyMessage;
use crate::session_fingerprint::tests::sample as fingerprint;

fn running_room() -> (Room, PeerKey, PeerKey) {
    let (a, s) = (PeerKey(7), PeerKey(2));
    let mut room = Room::new(fingerprint());
    room.add(a, cfg(SeatRole::Player, fingerprint()));
    room.add(s, cfg(SeatRole::Spectator, fingerprint()));
    room.start();
    for key in [HOST, a, s] {
        assert_eq!(room.phase(key), LobbyPhase::Running);
        room.take(key);
    }
    (room, a, s)
}

#[test]
fn resync_drops_every_driver_resyncs_and_rebuilds() {
    let (mut room, a, s) = running_room();
    room.event(HOST, LobbyEvent::ResyncRequested);
    for key in [HOST, a, s] {
        let effects = room.take(key);
        let drop_at = effects.iter().position(|e| *e == LobbyEffect::DropDriver);
        let rebuild_at = effects
            .iter()
            .position(|e| matches!(e, LobbyEffect::RebuildDriver(_)));
        assert!(
            drop_at.unwrap() < rebuild_at.unwrap(),
            "{key:?}: drop before rebuild"
        );
        assert!(!effects
            .iter()
            .any(|e| matches!(e, LobbyEffect::StartDriver(_))));
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, LobbyEffect::RunWarmup(_))),
            "no warm-up on resync"
        );
        let Some(LobbyEffect::RebuildDriver(plan)) = rebuild_at.map(|i| &effects[i]) else {
            unreachable!()
        };
        assert_eq!(plan.epoch, 1, "next ggrs epoch");
        assert_eq!(plan.handles.players(), &[(HOST, 0), (a, 1)], "same handles");
        if key != HOST {
            let ready = effects.iter().any(|e| {
                matches!(
                    e,
                    LobbyEffect::Send {
                        msg: LobbyMessage::ResyncReady,
                        ..
                    }
                )
            });
            assert!(ready, "{key:?} answered ResyncReady");
            assert!(effects
                .iter()
                .any(|e| matches!(e, LobbyEffect::LoadSync(_))));
        }
        assert_eq!(room.phase(key), LobbyPhase::Running);
    }
}

#[test]
fn missing_resync_ready_ends_the_host_after_five_seconds() {
    let (mut room, _a, s) = running_room();
    room.muted.push(s);
    room.event(HOST, LobbyEvent::ResyncRequested);
    assert_eq!(room.phase(HOST), LobbyPhase::Resyncing);
    room.later(Duration::from_secs(4));
    room.event(HOST, LobbyEvent::Tick);
    assert_eq!(room.phase(HOST), LobbyPhase::Resyncing);
    room.later(Duration::from_secs(1));
    room.event(HOST, LobbyEvent::Tick);
    assert!(matches!(
        room.phase(HOST),
        LobbyPhase::Ended(EndReason::Error(_))
    ));
}

#[test]
fn a_peer_leaving_during_resync_does_not_block_it() {
    let (mut room, a, s) = running_room();
    room.muted.push(s);
    room.event(HOST, LobbyEvent::ResyncRequested);
    room.muted.clear();
    room.leave(s);
    assert_eq!(room.phase(HOST), LobbyPhase::Running);
    assert_eq!(room.phase(a), LobbyPhase::Running);
    let plans = room.plans(HOST);
    let [LobbyEffect::RebuildDriver(plan)] = plans.as_slice() else {
        panic!("{plans:?}")
    };
    assert!(plan.handles.spectators().is_empty());
}

#[test]
fn lobby_pings_fill_the_roster_rtt() {
    let a = PeerKey(7);
    let mut room = Room::new(fingerprint());
    room.add(a, cfg(SeatRole::Player, fingerprint()));
    room.event(HOST, LobbyEvent::Tick); // Ping sent and answered at once (same instant)
    let rtt = room.node(HOST).1.roster()[1].rtt_ms;
    assert_eq!(rtt, Some(0));
    assert_eq!(room.node(a).1.roster()[1].rtt_ms, Some(0), "guests see it");
}

#[test]
fn quit_sends_leave_and_ends() {
    let (mut room, a, _s) = running_room();
    room.event(a, LobbyEvent::Quit);
    assert_eq!(room.phase(a), LobbyPhase::Ended(EndReason::Left));
    assert_eq!(
        room.node(HOST).1.roster().len(),
        2,
        "host dropped a from the roster"
    );
}

fn three_player_room() -> (Room, PeerKey, PeerKey) {
    let (a, b) = (PeerKey(7), PeerKey(3));
    let mut fp = fingerprint();
    fp.max_players = 4;
    let mut room = Room::new(fp.clone());
    room.add(a, cfg(SeatRole::Player, fp.clone()));
    room.add(b, cfg(SeatRole::Player, fp));
    room.start();
    for key in [HOST, a, b] {
        assert_eq!(room.phase(key), LobbyPhase::Running);
        room.take(key);
    }
    (room, a, b)
}

#[test]
fn resync_after_a_player_left_is_refused_without_dropping_drivers() {
    let (mut room, a, b) = three_player_room();
    room.leave(a); // handle 1: a rebuild would move b from port 2 to port 1
    room.take(HOST);
    room.event(HOST, LobbyEvent::ResyncRequested);
    assert_eq!(room.phase(HOST), LobbyPhase::Running);
    assert_eq!(room.phase(b), LobbyPhase::Running);
    assert!(!room.take(HOST).contains(&LobbyEffect::DropDriver));
    assert!(room.node(HOST).1.notice().unwrap().contains("resync"));
}

#[test]
fn a_player_leaving_during_resync_ends_instead_of_shifting_handles() {
    let (mut room, a, b) = three_player_room();
    room.muted.push(a);
    room.event(HOST, LobbyEvent::ResyncRequested);
    room.muted.clear();
    room.leave(a);
    assert!(matches!(
        room.phase(HOST),
        LobbyPhase::Ended(EndReason::Error(_))
    ));
    assert!(
        room.plans(HOST).is_empty(),
        "no rebuild with shifted handles"
    );
    assert!(matches!(room.phase(b), LobbyPhase::Ended(_)));
    assert!(room.plans(b).is_empty());
}

#[test]
fn resync_keeps_three_player_handles() {
    let (mut room, a, b) = three_player_room();
    room.event(HOST, LobbyEvent::ResyncRequested);
    let plans = room.plans(b);
    let [LobbyEffect::RebuildDriver(plan)] = plans.as_slice() else {
        panic!("{plans:?}")
    };
    assert_eq!(plan.handles.players(), &[(HOST, 0), (a, 1), (b, 2)]);
}
