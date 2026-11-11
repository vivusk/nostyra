#![allow(clippy::unwrap_used)]

use ggrs::NULL_FRAME;

use super::harness::{all_confirmed, configs, peers, run, HOST};
use super::*;
use crate::fake_socket::FakeNet;

fn same_at(peers: &[super::harness::Peer], frame: Frame) {
    let first = peers[0].host.after.get(&frame).copied();
    assert!(first.is_some(), "frame {frame} advanced");
    for peer in peers {
        assert_eq!(
            peer.host.after.get(&frame).copied(),
            first,
            "{:?} at {frame}",
            peer.key
        );
    }
}

#[test]
fn two_drivers_lossless_reach_the_same_state() {
    let net = FakeNet::new(0, 1, false);
    let mut p = peers(&net, &configs(1, 0, 2, 1));
    run(&net, &mut p, 5_000, |p| all_confirmed(p, 610));
    same_at(&p, 600);
    assert_ne!(p[0].host.after[&600], 0);
}

#[test]
fn loss_and_latency_still_converge_with_rollbacks() {
    let net = FakeNet::new(10, 3, true);
    let mut p = peers(&net, &configs(1, 0, 0, 1));
    run(&net, &mut p, 20_000, |p| all_confirmed(p, 610));
    same_at(&p, 600);
    assert!(p.iter().all(|peer| peer.resims > 0), "rollbacks happened");
    let stats = p[0].driver.stats();
    assert_eq!(stats.peers.len(), 1);
    assert_eq!(stats.peers[0].handle, 1);
}

#[test]
fn is_new_once_per_frame_and_correct_predictions_advance_once() {
    let net = FakeNet::new(0, 3, false);
    let mut p = peers(&net, &configs(1, 0, 0, 1));
    run(&net, &mut p, 5_000, |p| all_confirmed(p, 610));
    for peer in &p {
        let mut once = 0;
        for frame in 0..=600 {
            let flags = &peer.host.advances[&frame];
            assert!(flags[0], "first advance of {frame} is new");
            assert_eq!(flags.iter().filter(|&&new| new).count(), 1, "frame {frame}");
            once += usize::from(flags.len() == 1);
        }
        assert!(once > 0, "a correctly predicted frame is advanced once");
        assert!(peer.resims > 0);
        let (saves, grows) = peer.driver.ring_counters();
        assert!(
            saves > 600 && grows <= 10,
            "ring reuse: saves {saves} grows {grows}"
        );
    }
}

#[test]
fn a_silent_peer_makes_the_other_stall() {
    let net = FakeNet::new(0, 1, false);
    let mut p = peers(&net, &configs(1, 0, 0, 1));
    run(&net, &mut p, 5_000, |p| all_confirmed(p, 50));
    p[1].active = false;
    let before = p[0].stalls;
    run(&net, &mut p, 100, |p| p[0].stalls > before);
    assert_eq!(p[0].tick(), TickOutcome::Stalled);
    assert!(p[0].driver.stats().stall_count > 0);
}

#[test]
fn spectator_follows_the_host() {
    let net = FakeNet::new(0, 2, false);
    let mut p = peers(&net, &configs(1, 1, 1, 1));
    run(&net, &mut p, 10_000, |p| all_confirmed(p, 310));
    same_at(&p, 300);
    let spectator = &p[2];
    assert_eq!(
        spectator.driver.confirmed_frame(),
        spectator.driver.head_frame()
    );
    assert!(
        spectator
            .host
            .advances
            .values()
            .all(|flags| flags == &[true]),
        "spectators never re-simulate"
    );
}

#[test]
fn desync_is_detected_within_two_intervals() {
    let net = FakeNet::new(0, 1, false);
    let mut p = peers(&net, &configs(1, 0, 1, 1));
    p[1].host.corrupt_from = Some(130);
    let mut events = Vec::new();
    for _ in 0..5_000 {
        net.tick();
        if p.iter().any(|peer| !peer.driver.is_running()) {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        p.iter_mut().for_each(|peer| {
            let _ = peer.tick();
        });
        events.extend(p[0].driver.drain_events());
        if all_confirmed(&p, 130 + 2 * 120) {
            break;
        }
    }
    let desync = events.iter().find_map(|e| match e {
        NetplayEvent::DesyncDetected { frame, peer } => Some((*frame, *peer)),
        _ => None,
    });
    let (frame, peer) = desync.expect("DesyncDetected");
    assert!((130..=130 + 2 * 120).contains(&frame), "{frame}");
    assert_eq!(peer, p[1].key);
    assert!(p[0].driver.stats().desync);
}

#[test]
fn rebuild_starts_a_second_session_on_the_same_channel() {
    let net = FakeNet::new(0, 2, false);
    let mut p = peers(&net, &configs(1, 0, 1, 1));
    run(&net, &mut p, 5_000, |p| all_confirmed(p, 300));
    let second = configs(1, 0, 1, 2);
    // The host rebuilds first while the other peer's old session keeps sending
    // (`endpoint(key)` = a handle on the same per-key channel, like `ggrs_channel()`).
    p[0].driver.rebuild(&second[0], net.endpoint(HOST)).unwrap();
    p[0].host = Default::default();
    assert_eq!(p[0].driver.head_frame(), NULL_FRAME);
    for _ in 0..30 {
        net.tick();
        p.iter_mut().for_each(|peer| {
            let _ = peer.tick();
        });
    }
    let channel = net.endpoint(p[1].key);
    p[1].driver.rebuild(&second[1], channel).unwrap();
    p[1].host = Default::default();
    assert_eq!(p[1].driver.head_frame(), NULL_FRAME);
    assert_eq!(p[1].driver.stats().stall_count, 0, "stats reset");
    run(&net, &mut p, 10_000, |p| all_confirmed(p, 310));
    same_at(&p, 300);
}

#[test]
fn not_synchronized_yet_is_waiting() {
    let net = FakeNet::new(0, 5, false);
    let mut p = peers(&net, &configs(1, 0, 1, 1));
    assert_eq!(p[0].tick(), TickOutcome::Waiting);
    assert!(!p[0].driver.is_running());
    assert_eq!(p[0].driver.confirmed_frame(), NULL_FRAME);
}
