//! `AchievementSession` progress in states (round trip, absent → reset, pending while
//! the set loads, rewind playback), no-session cases and the disc swap (own hash →
//! change media).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::cell::Cell;
use std::path::Path;

use achievements::RequestDispatch;
use config_layers::AchievementsConfig;

use super::achievement_session_test_support::*;
use super::progress_of;

#[test]
fn state_progress_round_trips_and_absent_progress_resets() {
    let mut h = harness(fake_with_set(), RequestDispatch::Inline);
    h.frame(); // set loads, achievements arm while the condition is false
    h.ram[0x30].set(1);
    h.frames(5); // 5 of 10 hits
    let saved = progress_of(&Some(h.session)).expect("progress");
    let mut h = harness(fake_with_set(), RequestDispatch::Inline);
    h.session.apply_loaded_progress(Some(&saved));
    h.ram[0x30].set(1);
    h.frames(4);
    assert!(h.unlocked().is_empty(), "restored 5 + 4 hits");
    h.frame();
    assert_eq!(h.unlocked(), [HITS_ACH]);

    let mut h = harness(fake_with_set(), RequestDispatch::Inline);
    h.frame();
    h.ram[0x30].set(1);
    h.frames(5);
    h.session.apply_loaded_progress(None);
    h.ram[0x30].set(0);
    h.frame(); // reset achievements re-arm while false
    h.ram[0x30].set(1);
    h.frames(9);
    assert!(
        h.unlocked().is_empty(),
        "state without progress resets hits"
    );
    h.frame();
    assert_eq!(h.unlocked(), [HITS_ACH]);
}

#[test]
fn rewind_playback_restores_the_captured_progress() {
    let mut h = harness(fake_with_set(), RequestDispatch::Inline);
    h.frame();
    h.ram[0x30].set(1);
    h.frames(5); // 5 of 10 hits: the capture
    let blob = h.session.rewind_progress().expect("progress blob");
    h.frames(3); // 8 hits
    h.session.restore_rewind_progress(Some(&blob)); // playback lands on the capture
    h.frames(4);
    assert!(h.unlocked().is_empty(), "back to 5 hits, 9 now");
    h.frame();
    assert_eq!(h.unlocked(), [HITS_ACH]);
}

#[test]
fn progress_loaded_while_the_set_is_loading_applies_when_it_completes() {
    let mut first = harness(fake_with_set(), RequestDispatch::Inline);
    first.frame();
    first.ram[0x30].set(1);
    first.frames(5);
    let saved = progress_of(&Some(first.session)).unwrap();
    let mut h = harness(fake_with_set(), RequestDispatch::Worker);
    assert!(!h.loaded(), "the load starts after frame 1");
    h.session.apply_loaded_progress(Some(&saved)); // launcher Continue before frame 1
    h.wait_loaded();
    h.ram[0x30].set(1);
    h.frames(4);
    assert!(h.unlocked().is_empty(), "pending progress applied on load");
    h.frame();
    assert_eq!(h.unlocked(), [HITS_ACH]);
    h.session.shutdown();
}

#[test]
fn no_session_when_off_logged_out_or_unsupported() {
    let data = tempfile::tempdir().unwrap();
    let rom = nes_rom(data.path());
    let fake = fake_with_set();
    let nes = (&system_catalog::NES, rom.as_path());
    let off = AchievementsConfig {
        enabled: false,
        ..Default::default()
    };
    let on = AchievementsConfig::default();
    let dispatch = RequestDispatch::Inline;
    assert!(try_start(&fake, dispatch, nes, &off, data.path(), login())
        .0
        .is_none());
    assert!(
        try_start(&fake, dispatch, nes, &on, data.path(), None)
            .0
            .is_none(),
        "logged out"
    );
    let gc = (&system_catalog::GC, rom.as_path());
    assert!(try_start(&fake, dispatch, gc, &on, data.path(), login())
        .0
        .is_none());
    assert!(fake.posts().is_empty(), "zero server traffic");
    let pbp = data.path().join("game.pbp");
    std::fs::write(&pbp, b"\0PBP").unwrap();
    let psx = (&system_catalog::PSX, pbp.as_path());
    let (inert, _) = try_start(&fake, dispatch, psx, &on, data.path(), login());
    let notices = inert.expect("inert session with a notice").take_notices();
    assert!(
        notices[0].contains("not available for this format"),
        "{notices:?}"
    );
}

#[test]
fn disc_swap_hashes_the_new_disc_and_changes_media() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms");
    let data = tempfile::tempdir().unwrap();
    let fake = fake_with_set();
    let m3u = repo.join("nostyra-test-psx.m3u");
    let config = AchievementsConfig::default();
    let disc = (&system_catalog::PSX, m3u.as_path());
    let dispatch = RequestDispatch::Inline;
    let (session, _) = try_start(&fake, dispatch, disc, &config, data.path(), login());
    let mut session = session.unwrap();
    let ram: Vec<Cell<u8>> = (0..0x20_0000).map(|_| Cell::new(0)).collect();
    let lookup = |id: u32| (id == 2).then_some((ram.as_ptr() as *mut u8, ram.len()));
    session.on_real_frame(None, &lookup); // disc 1 identified
    session.disc_swapped(1);
    assert!(
        session.memory.borrow().regions().is_none(),
        "regions dropped on swap"
    );
    session.on_real_frame(None, &lookup); // regions rebuilt → change media
    let disc2 = repo.join("nostyra-test-psx-disc2.cue");
    let hash = achievements::content_hash(12, &disc2, None).unwrap();
    // rc_client resolves an unseen hash with the server (`r=gameid&m=<hash>`).
    let posts = fake.posts();
    let lookup_post = posts.iter().rfind(|p| p.contains("&m=")).unwrap();
    assert!(lookup_post.contains(&format!("m={hash}")), "{posts:?}");
    session.shutdown();
}
