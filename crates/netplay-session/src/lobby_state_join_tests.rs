#![allow(clippy::unwrap_used)]

use super::harness::{cfg, refusal, Room, HOST};
use super::*;
use crate::lobby_protocol::{LobbyMessage, RefusalReason};
use crate::session_fingerprint::tests::sample as fingerprint;
use crate::session_fingerprint::SessionFingerprint;

#[test]
fn mismatch_refuses_with_the_field() {
    let a = PeerKey(7);
    let mut room = Room::new(fingerprint());
    room.add(
        a,
        cfg(
            SeatRole::Player,
            SessionFingerprint {
                core_version: "1.63".into(),
                ..fingerprint()
            },
        ),
    );
    assert_eq!(
        refusal(&room, a).text(),
        "Different core version: 1.62 vs 1.63"
    );
    assert_eq!(room.node(HOST).1.roster().len(), 1);
    assert!(!room.node(HOST).1.can_start());
}

#[test]
fn duplicate_hello_is_ignored() {
    let a = PeerKey(7);
    let mut room = Room::new(fingerprint());
    room.add(a, cfg(SeatRole::Player, fingerprint()));
    room.take(HOST);
    let hello = room.node(a).1.hello();
    room.event(HOST, LobbyEvent::Message(a, hello));
    assert!(room.take(HOST).is_empty());
    assert_eq!(room.node(HOST).1.roster().len(), 2);
}

#[test]
fn hello_after_start_is_refused_as_started() {
    let (a, late) = (PeerKey(7), PeerKey(8));
    let mut room = Room::new(fingerprint());
    room.add(a, cfg(SeatRole::Player, fingerprint()));
    room.muted.push(a);
    room.start(); // host waits in Syncing
    room.add(late, cfg(SeatRole::Spectator, fingerprint()));
    assert_eq!(refusal(&room, late), RefusalReason::Started);
    room.muted.clear();
    room.event(HOST, LobbyEvent::Message(a, LobbyMessage::SyncAck));
    assert_eq!(room.phase(HOST), LobbyPhase::Running);
    let late2 = PeerKey(9);
    room.add(late2, cfg(SeatRole::Player, fingerprint()));
    assert_eq!(refusal(&room, late2), RefusalReason::Started);
}

#[test]
fn caps_refuse_fifth_player_and_ninth_spectator() {
    let fp4 = SessionFingerprint {
        max_players: 4,
        ..fingerprint()
    };
    let mut room = Room::new(fp4.clone());
    for k in 1..=3 {
        room.add(PeerKey(k), cfg(SeatRole::Player, fp4.clone()));
    }
    room.add(PeerKey(4), cfg(SeatRole::Player, fp4.clone()));
    assert_eq!(refusal(&room, PeerKey(4)), RefusalReason::RoomFull);
    for k in 10..18 {
        room.add(PeerKey(k), cfg(SeatRole::Spectator, fp4.clone()));
        assert_eq!(room.phase(PeerKey(k)), LobbyPhase::InLobby);
    }
    room.add(PeerKey(18), cfg(SeatRole::Spectator, fp4));
    assert_eq!(refusal(&room, PeerKey(18)), RefusalReason::RoomFull);
}

#[test]
fn guest_ignores_other_guests_hello_and_follows_the_welcome_sender() {
    let (a, b) = (PeerKey(7), PeerKey(3));
    let mut room = Room::new(fingerprint());
    room.add(a, cfg(SeatRole::Player, fingerprint()));
    room.add(b, cfg(SeatRole::Spectator, fingerprint()));
    let hello_from_b = room.node(b).1.hello();
    room.take(a);
    room.event(a, LobbyEvent::Message(b, hello_from_b));
    assert!(room.take(a).is_empty(), "non-hosts ignore Hello");
    assert_eq!(room.node(a).1.host(), Some(HOST));
    assert_eq!(room.node(b).1.host(), Some(HOST));
}

#[test]
fn sram_size_mismatch_ends_the_guest() {
    let a = PeerKey(7);
    let mut room = Room::new(fingerprint());
    room.add(
        a,
        LobbyConfig {
            sram_len: 8192,
            ..cfg(SeatRole::Player, fingerprint())
        },
    );
    room.start();
    assert_eq!(
        refusal(&room, a),
        RefusalReason::SramSize {
            host: 4,
            guest: 8192
        }
    );
    assert_eq!(
        room.node(HOST).1.roster().len(),
        1,
        "the guest's Leave reached the host"
    );
    assert_eq!(room.phase(HOST), LobbyPhase::InLobby);
}

#[test]
fn undecodable_hello_refuses_the_peer_and_keeps_start_possible() {
    let (a, x) = (PeerKey(7), PeerKey(8));
    let mut room = Room::new(fingerprint());
    room.add(a, cfg(SeatRole::Player, fingerprint()));
    room.event(HOST, LobbyEvent::PeerJoined(x));
    assert!(
        !room.node(HOST).1.can_start(),
        "un-greeted peer blocks Start"
    );
    room.take(HOST);
    let err = crate::NetplayError::ProtocolVersion {
        found: 2,
        expected: 1,
    };
    room.event(HOST, LobbyEvent::BadMessage(x, err));
    assert!(room.node(HOST).1.can_start());
    assert!(room
        .node(HOST)
        .1
        .notice()
        .unwrap()
        .contains("update Nostyra"));
    let refused = LobbyEffect::Send {
        to: vec![x],
        msg: LobbyMessage::Refused {
            reason: RefusalReason::Protocol,
        },
    };
    assert!(room.take(HOST).contains(&refused));
}

#[test]
fn undecodable_reply_ends_a_handshaking_guest() {
    let a = PeerKey(7);
    let mut guest = LobbyState::new(cfg(SeatRole::Player, fingerprint()));
    let now = std::time::Instant::now();
    guest.apply(LobbyEvent::PeerJoined(HOST), now);
    guest.apply(LobbyEvent::Connected { me: a }, now);
    assert_eq!(guest.phase(), &LobbyPhase::Handshaking);
    let err = crate::NetplayError::ProtocolVersion {
        found: 2,
        expected: 1,
    };
    guest.apply(LobbyEvent::received(HOST, Err(err)), now);
    let LobbyPhase::Ended(EndReason::Error(text)) = guest.phase() else {
        panic!("{:?}", guest.phase())
    };
    assert!(text.contains("update Nostyra"));
}
