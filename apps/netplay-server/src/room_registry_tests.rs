use std::time::{Duration, Instant};

use matchbox_protocol::PeerId;

use super::*;

pub(crate) fn registry() -> RoomRegistry {
    RoomRegistry::new(RoomLimits {
        max_rooms: 3,
        max_spectators: 2,
    })
}

pub(crate) fn new_room(max_players: u8, allow_spectators: bool) -> NewRoom {
    NewRoom {
        nickname: "host".into(),
        game: GameHint {
            content_sha1: "a".repeat(40),
            title: "Test".into(),
            system: "snes".into(),
            core_id: "snes9x".into(),
            core_version: "1.62".into(),
        },
        max_players,
        allow_spectators,
    }
}

pub(crate) fn live() -> PeerId {
    PeerId(uuid::Uuid::new_v4())
}

pub(crate) fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn create_join_full_start_late_join_gc() {
    let reg = registry();
    let t0 = Instant::now();
    let host = reg.create(new_room(4, true), t0).unwrap();
    assert_eq!(host.role, SeatRole::Host);
    for i in 0..3 {
        let p = reg
            .join(&host.code, format!("p{i}"), SeatRole::Player, t0)
            .unwrap();
        assert_eq!(
            p.max_players, 4,
            "join carries the room's max_players (F21)"
        );
        assert_eq!(p.game, host.game);
        assert_eq!(p.roster.len(), i + 2);
    }
    let fourth = reg.join(&host.code, "p4".into(), SeatRole::Player, t0);
    assert_eq!(fourth.err(), Some(RegistryError::RoomFull));
    reg.start(&host.code, &host.room_id, SeatRole::Host)
        .unwrap();
    let late = reg.join(&host.code, "late".into(), SeatRole::Spectator, t0);
    assert_eq!(late.err(), Some(RegistryError::Started));
    assert_eq!(
        reg.collect_garbage(t0 + secs(5 * 60)),
        1,
        "nobody ever connected"
    );
    assert_eq!(
        reg.status(&host.code, t0).err(),
        Some(RegistryError::NotFound)
    );
}

#[test]
fn start_needs_the_host_token_of_that_room() {
    let reg = registry();
    let now = Instant::now();
    let a = reg.create(new_room(2, false), now).unwrap();
    let b = reg.create(new_room(2, false), now).unwrap();
    let p = reg
        .join(&a.code, "p".into(), SeatRole::Player, now)
        .unwrap();
    assert_eq!(
        reg.start(&a.code, &p.room_id, SeatRole::Player),
        Err(RegistryError::NotHost)
    );
    assert_eq!(
        reg.start(&a.code, &b.room_id, SeatRole::Host),
        Err(RegistryError::NotHost)
    );
    assert_eq!(
        reg.start("ZZZZZZ", &a.room_id, SeatRole::Host),
        Err(RegistryError::NotFound)
    );
    reg.start(&a.code, &a.room_id, SeatRole::Host).unwrap();
}

#[test]
fn spectators_off_cap_and_room_cap() {
    let reg = registry();
    let now = Instant::now();
    let off = reg.create(new_room(2, false), now).unwrap();
    let spec = reg.join(&off.code, "s".into(), SeatRole::Spectator, now);
    assert_eq!(spec.err(), Some(RegistryError::SpectatorsOff));
    let on = reg.create(new_room(2, true), now).unwrap();
    reg.join(&on.code, "s1".into(), SeatRole::Spectator, now)
        .unwrap();
    reg.join(&on.code, "s2".into(), SeatRole::Spectator, now)
        .unwrap();
    let s3 = reg.join(&on.code, "s3".into(), SeatRole::Spectator, now);
    assert_eq!(s3.err(), Some(RegistryError::RoomFull));
    reg.create(new_room(2, true), now).unwrap();
    assert_eq!(
        reg.create(new_room(2, true), now).err(),
        Some(RegistryError::TooManyRooms)
    );
}

#[test]
fn expired_room_refuses_joins_and_connects() {
    let reg = registry();
    let t0 = Instant::now();
    let host = reg.create(new_room(2, false), t0).unwrap();
    let later = t0 + ROOM_LIFETIME;
    let join = reg.join(&host.code, "p".into(), SeatRole::Player, later);
    assert_eq!(join.err(), Some(RegistryError::Expired));
    let connect = reg.connect(&host.room_id, &host.peer_id, live(), later);
    assert_eq!(connect.err(), Some(RegistryError::Expired));
}

#[test]
fn connect_caps_count_live_connections_f28() {
    let reg = registry();
    let t0 = Instant::now();
    let host = reg.create(new_room(2, false), t0).unwrap();
    let stale = reg
        .join(&host.code, "stale".into(), SeatRole::Player, t0)
        .unwrap();
    // The stale seat's place lapses after 5 min unconnected; a new player takes it.
    let t1 = t0 + PENDING_SEAT_TTL;
    reg.connect(&host.room_id, &host.peer_id, live(), t0)
        .unwrap();
    let fresh = reg
        .join(&host.code, "fresh".into(), SeatRole::Player, t1)
        .unwrap();
    assert_eq!(
        reg.connect(&host.room_id, &fresh.peer_id, live(), t1),
        Ok(SeatRole::Player)
    );
    // The lapsed seat's token is still valid, but the live cap refuses it.
    let over = reg.connect(&host.room_id, &stale.peer_id, live(), t1);
    assert_eq!(over.err(), Some(RegistryError::RoomFull));
    let again = reg.connect(&host.room_id, &fresh.peer_id, live(), t1);
    assert_eq!(again.err(), Some(RegistryError::AlreadyConnected));
    let status = reg.status(&host.code, t1).unwrap();
    assert_eq!(
        (status.players, status.spectators, status.max_players),
        (2, 0, 2)
    );
}

#[test]
fn connect_refuses_a_player_over_the_live_cap() {
    let reg = registry();
    let t0 = Instant::now();
    let host = reg.create(new_room(2, false), t0).unwrap();
    let a = reg
        .join(&host.code, "a".into(), SeatRole::Player, t0)
        .unwrap();
    reg.connect(&host.room_id, &host.peer_id, live(), t0)
        .unwrap();
    reg.connect(&host.room_id, &a.peer_id, live(), t0).unwrap();
    // `a` leaves and rejoins with a new seat while its old one reconnects too.
    reg.disconnect(&host.room_id, &a.peer_id, t0);
    let b = reg
        .join(&host.code, "b".into(), SeatRole::Player, t0)
        .unwrap();
    reg.connect(&host.room_id, &b.peer_id, live(), t0).unwrap();
    let over = reg.connect(&host.room_id, &a.peer_id, live(), t0);
    assert_eq!(
        over.err(),
        Some(RegistryError::RoomFull),
        "3rd live player in a 2-player room"
    );
    assert_eq!(reg.status(&host.code, t0).unwrap().players, 2);
}
