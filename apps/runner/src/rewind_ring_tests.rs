//! `RewindRing` unit tests on synthetic 800 KiB states with sparse changes.

use super::RewindRing;

const STATE_LEN: usize = 800 * 1024;

/// State `n`: a fixed pseudo-random background with a few bytes changed per step,
/// like consecutive emulator frames.
fn state(n: u32) -> Vec<u8> {
    let mut s: Vec<u8> = (0..STATE_LEN).map(|i| (i * 31 % 251) as u8).collect();
    for k in 0..64u32 {
        let at = ((k * 12_289 + n * 977) as usize) % STATE_LEN;
        s[at] = s[at].wrapping_add(n as u8).wrapping_add(1);
    }
    s[..4].copy_from_slice(&n.to_le_bytes());
    s
}

fn filled(budget: usize, count: u32) -> RewindRing {
    let mut ring = RewindRing::new(budget);
    for n in 1..=count {
        ring.push(&state(n));
    }
    ring
}

#[test]
fn pops_walk_back_to_the_oldest_capture() {
    let mut ring = filled(64 << 20, 10);
    assert_eq!(ring.len(), 9);
    assert_eq!(ring.current(), Some(state(10).as_slice()));
    for n in (1..10).rev() {
        assert!(ring.pop() == Some(state(n).as_slice()), "pop → state {n}");
    }
    assert!(ring.pop().is_none(), "start of buffer");
    assert_eq!(ring.current(), Some(state(1).as_slice()), "oldest held");
    assert_eq!(ring.len(), 0);
}

#[test]
fn push_after_pops_continues_from_the_rewound_point() {
    let mut ring = filled(64 << 20, 5);
    ring.pop();
    ring.pop(); // current = state 3
    ring.push(&state(42));
    assert_eq!(ring.len(), 3);
    assert!(ring.pop() == Some(state(3).as_slice()));
    assert!(ring.pop() == Some(state(2).as_slice()));
}

#[test]
fn deltas_compress_far_below_the_raw_size() {
    let ring = filled(64 << 20, 11);
    let per_entry = (ring.bytes_used() - STATE_LEN) / ring.len();
    assert!(per_entry < STATE_LEN / 50, "entry {per_entry} B");
}

#[test]
fn budget_evicts_oldest_and_keeps_newest() {
    let probe = filled(usize::MAX, 2);
    let entry = probe.bytes_used() - STATE_LEN;
    // Room for the raw state + 3 entries.
    let mut ring = filled(STATE_LEN + 3 * entry + entry / 2, 10);
    assert_eq!(ring.len(), 3);
    assert!(ring.bytes_used() <= STATE_LEN + 3 * entry + entry / 2);
    for n in [9, 8, 7] {
        assert!(ring.pop() == Some(state(n).as_slice()), "newest kept: {n}");
    }
    assert!(ring.pop().is_none());
}

#[test]
fn state_size_change_restarts_history() {
    let mut ring = filled(64 << 20, 4);
    let bigger = vec![7u8; STATE_LEN + 16];
    ring.push(&bigger);
    assert_eq!(ring.len(), 0);
    assert_eq!(ring.current(), Some(bigger.as_slice()));
    assert_eq!(ring.bytes_used(), bigger.len());
}

#[test]
fn corrupt_entry_clears_and_returns_none() {
    let mut ring = filled(64 << 20, 4);
    let entry = ring.newest_entry_mut().unwrap();
    entry.iter_mut().for_each(|b| *b = 0xFF);
    assert!(ring.pop().is_none());
    assert_eq!(ring.current(), None, "never hands back garbage");
    assert_eq!((ring.len(), ring.bytes_used()), (0, 0));
}

#[test]
fn clear_forgets_everything() {
    let mut ring = filled(64 << 20, 3);
    ring.clear();
    assert_eq!(
        (ring.len(), ring.bytes_used(), ring.current()),
        (0, 0, None)
    );
    assert!(ring.pop().is_none());
}

#[test]
fn achievement_progress_follows_pops_and_counts_in_the_budget() {
    let blob = |n: u32| vec![n as u8; 300];
    let mut ring = RewindRing::new(64 << 20);
    ring.push_with_progress(&state(1), Some(&blob(1)));
    ring.push_with_progress(&state(2), None);
    ring.push_with_progress(&state(3), Some(&blob(3)));
    let without = filled(64 << 20, 3).bytes_used();
    assert_eq!(ring.bytes_used(), without + 600, "both blobs budgeted");
    assert_eq!(ring.current_progress(), Some(&blob(3)[..]));
    ring.pop();
    assert_eq!(
        ring.current_progress(),
        None,
        "capture 2 had no session progress"
    );
    ring.pop();
    assert_eq!(ring.current_progress(), Some(&blob(1)[..]));
    assert_eq!(ring.bytes_used(), STATE_LEN + 300);
    ring.clear();
    assert_eq!((ring.current_progress(), ring.bytes_used()), (None, 0));
}
