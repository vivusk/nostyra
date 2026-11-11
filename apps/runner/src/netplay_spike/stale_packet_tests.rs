//! Phase-00 F20 / F23 on an in-memory delayed socket (ggrs logic is transport
//! independent): re-advance counts per frame, and a second `P2PSession` built on the
//! same channel while the peer's old session still sends — with and without a
//! session-epoch filter in the socket wrapper. Prints `F20` / `F23` lines.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::panic::{catch_unwind, AssertUnwindSafe};

use super::mem_link_fixture_tests::{link, play, session, tick, Game};

#[test]
fn f20_readvance_counts() {
    let link = link();
    let (mut a, mut b) = (session(&link, 0, 0, false), session(&link, 1, 0, false));
    let (mut ga, gb) = play(&link, &mut a, &mut b, 600);
    assert_eq!(
        ga.after.get(&600),
        gb.after.get(&600),
        "same confirmed frame 600"
    );
    let (once, more) = ga.report();
    eprintln!(
        "F20 frames={} once={once} more_than_once={more} predicted_in_only_advance={}",
        ga.advances.len(),
        ga.predicted_single
    );
    assert!(more > 0, "scripted remote input must cause rollbacks");
    assert!(
        ga.predicted_single > 0,
        "a correctly predicted frame is never re-advanced"
    );
}

/// Peer A rebuilds first while B's old session keeps sending for 30 ticks; true when
/// both new sessions confirm frame 300 with equal hashes.
fn rebuild(filter: bool) -> bool {
    let link = link();
    let (mut a, mut b) = (session(&link, 0, 1, filter), session(&link, 1, 1, filter));
    play(&link, &mut a, &mut b, 300);
    drop(a);
    let mut a2 = session(&link, 0, 2, filter);
    let (mut ga2, mut gb_old) = (Game::default(), Game::default());
    for _ in 0..30 {
        link.clock.set(link.clock.get() + 1);
        tick(&mut b, &mut gb_old, 1);
        tick(&mut a2, &mut ga2, 0);
    }
    drop(b);
    let mut b2 = session(&link, 1, 2, filter);
    let (ga, gb) = play(&link, &mut a2, &mut b2, 300);
    let synced = a2.confirmed_frame() >= 300 && b2.confirmed_frame() >= 300;
    synced && ga.after.contains_key(&300) && ga.after.get(&300) == gb.after.get(&300)
}

fn second_session(filter: bool) -> Result<bool, String> {
    catch_unwind(AssertUnwindSafe(|| rebuild(filter))).map_err(|panic| {
        let text = panic.downcast_ref::<String>().cloned();
        text.or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default()
    })
}

#[test]
fn f23_second_session_on_same_channel() {
    for filter in [false, true] {
        let outcome = second_session(filter);
        eprintln!("F23 epoch_filter={filter} outcome={outcome:?}");
        if filter {
            assert_eq!(
                outcome,
                Ok(true),
                "with the epoch filter the rebuild must sync"
            );
        }
    }
}
