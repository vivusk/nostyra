use std::time::Instant;

use crate::room_model::ROOM_LIFETIME;
use crate::room_registry::tests::{live, new_room, registry, secs};

#[test]
fn gc_never_connected_room_after_five_minutes() {
    let reg = registry();
    let t0 = Instant::now();
    reg.create(new_room(2, false), t0).unwrap();
    assert_eq!(reg.collect_garbage(t0 + secs(4 * 60)), 0);
    assert_eq!(reg.room_count(), 1);
    assert_eq!(reg.collect_garbage(t0 + secs(5 * 60)), 1);
    assert_eq!(reg.room_count(), 0);
}

#[test]
fn gc_empty_room_sixty_seconds_after_last_peer_left() {
    let reg = registry();
    let t0 = Instant::now();
    let host = reg.create(new_room(2, false), t0).unwrap();
    reg.connect(&host.room_id, &host.peer_id, live(), t0 + secs(10))
        .unwrap();
    assert_eq!(
        reg.collect_garbage(t0 + secs(20 * 60)),
        0,
        "live peer keeps the room"
    );
    let left = t0 + secs(20 * 60);
    reg.disconnect(&host.room_id, &host.peer_id, left);
    assert_eq!(reg.collect_garbage(left + secs(59)), 0);
    assert_eq!(reg.collect_garbage(left + secs(60)), 1);
}

#[test]
fn gc_keeps_an_expired_room_while_peers_are_live() {
    let reg = registry();
    let t0 = Instant::now();
    let host = reg.create(new_room(2, false), t0).unwrap();
    reg.connect(&host.room_id, &host.peer_id, live(), t0)
        .unwrap();
    let late = t0 + ROOM_LIFETIME + secs(60);
    assert_eq!(reg.collect_garbage(late), 0);
    reg.disconnect(&host.room_id, &host.peer_id, late);
    assert_eq!(reg.collect_garbage(late), 1);
}
