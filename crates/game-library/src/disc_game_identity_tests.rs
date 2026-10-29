//! Disc identity tests: hashed file per layout, serial source.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use disc_image::test_images::{ps1_sectors, to_iso, to_raw, write, US_LICENCE};

use super::*;

fn disc(boot: &str) -> Vec<u8> {
    to_raw(&ps1_sectors(Some(US_LICENCE), boot), 2)
}

#[test]
fn cue_and_playlist_hash_the_first_data_track() {
    let dir = tempfile::tempdir().unwrap();
    let track1 = write(
        dir.path(),
        "G (Track 1).bin",
        &disc(r"BOOT = cdrom:\SLUS_012.34;1"),
    );
    write(dir.path(), "G (Track 2).bin", &[7u8; 2352 * 3]);
    let cue = write(
        dir.path(),
        "G.cue",
        b"FILE \"G (Track 1).bin\" BINARY\n TRACK 01 MODE2/2352\n  INDEX 01 00:00:00\nFILE \"G (Track 2).bin\" BINARY\n TRACK 02 AUDIO\n  INDEX 01 00:00:00\n",
    );
    let want = DiscIdentity {
        hashes: hash_file_streaming(&track1).unwrap(),
        serial: Some("SLUS-01234".into()),
        title: None,
    };
    assert_eq!(
        read_disc_identity(&cue, DiscSignature::PlayStation).unwrap(),
        want
    );
    let m3u = write(dir.path(), "G.m3u", b"G.cue\nother.cue\n");
    assert_eq!(
        read_disc_identity(&m3u, DiscSignature::PlayStation).unwrap(),
        want
    );
    assert_eq!(
        read_disc_identity(&track1, DiscSignature::PlayStation).unwrap(),
        want
    );
}

#[test]
fn iso_without_system_cnf_has_no_serial_and_errors_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let sectors = disc_image::test_images::iso_sectors("PLAYSTATION", Some(US_LICENCE), &[]);
    let iso = write(dir.path(), "x.iso", &to_iso(&sectors));
    assert_eq!(
        read_disc_identity(&iso, DiscSignature::PlayStation)
            .unwrap()
            .serial,
        None
    );
    let empty = write(dir.path(), "e.m3u", b"# nothing\n");
    assert!(read_disc_identity(&empty, DiscSignature::PlayStation).is_err());
}
