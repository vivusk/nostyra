//! Identity source tests: the same sheet hashed under each signature's rule.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::test_gd_images::{write_gdi, write_redump_gd_cue};
use crate::test_images::write;

#[test]
fn dreamcast_sheets_hash_the_largest_data_track() {
    let dir = tempfile::tempdir().unwrap();
    let gdi = write_gdi(dir.path(), "g", "NSTY-00001", false);
    let dc = |p: &Path| identity_source(p, DiscSignature::Dreamcast).unwrap();
    assert_eq!(
        dc(&gdi),
        dir.path().join("g-track03.bin"),
        "never the sheet text"
    );
    let cue = write_redump_gd_cue(dir.path(), "Game", "NSTY-00001");
    assert_eq!(dc(&cue), dir.path().join("Game (Track 3).bin"));
    let m3u = write(dir.path(), "two.m3u", b"g.gdi\nGame.cue\n");
    assert_eq!(
        dc(&m3u),
        dir.path().join("g-track03.bin"),
        "first disc's rule"
    );
    let chd = write(dir.path(), "x.chd", b"x");
    assert_eq!(dc(&chd), chd, "containers hash whole");
}

#[test]
fn the_same_cue_follows_each_signatures_rule() {
    let dir = tempfile::tempdir().unwrap();
    let cue = write_redump_gd_cue(dir.path(), "Game", "NSTY-00001");
    let ps1 = identity_source(&cue, DiscSignature::PlayStation).unwrap();
    assert_eq!(
        ps1,
        dir.path().join("Game (Track 1).bin"),
        "PS1: first data track"
    );
    let dc = identity_source(&cue, DiscSignature::Dreamcast).unwrap();
    assert_eq!(dc, dir.path().join("Game (Track 3).bin"));
}

#[test]
fn equal_sizes_pick_the_last_and_missing_tracks_fail() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.bin", &[0; 2048]);
    write(dir.path(), "b.bin", &[0; 2048]);
    let gdi = write(
        dir.path(),
        "t.gdi",
        b"2\n1 0 4 2048 a.bin 0\n2 45000 4 2048 b.bin 0\n",
    );
    let pick = identity_source(&gdi, DiscSignature::Dreamcast).unwrap();
    assert_eq!(pick, dir.path().join("b.bin"));
    let broken = write(dir.path(), "m.gdi", b"1\n1 0 4 2048 gone.bin 0\n");
    let err = identity_source(&broken, DiscSignature::Dreamcast).unwrap_err();
    assert!(matches!(err, DiscError::MissingFile(name) if name == "gone.bin"));
}
