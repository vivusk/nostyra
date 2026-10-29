//! CHD tests: hunk math, track metadata, reading a synthetic uncompressed CHD v5.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::disc_sector_reader::open_data_track;
use crate::test_images::{iso_sectors, to_chd, to_raw, write};

#[test]
fn frame_location_splits_into_hunks() {
    let hunk = CHD_FRAME_BYTES * 8;
    assert_eq!(frame_location(0, hunk), (0, 0));
    assert_eq!(frame_location(7, hunk), (0, 7 * 2448));
    assert_eq!(frame_location(16, hunk), (2, 0));
    assert_eq!(frame_location(17, hunk), (2, 2448));
}

#[test]
fn track_layout_from_metadata() {
    let raw =
        TrackLayout::parse("TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE FRAMES:300 PREGAP:0 PGTYPE:MODE1")
            .unwrap();
    assert_eq!(
        raw,
        TrackLayout {
            data_offset: 24,
            first_frame: 0,
            frames: 300
        }
    );
    let cooked =
        TrackLayout::parse("TRACK:1 TYPE:MODE1 FRAMES:300 PREGAP:150 PGTYPE:VMODE1").unwrap();
    assert_eq!(
        cooked,
        TrackLayout {
            data_offset: 0,
            first_frame: 150,
            frames: 150
        }
    );
    assert!(TrackLayout::parse("TRACK:1 TYPE:AUDIO FRAMES:10").is_err());
}

#[test]
fn reads_sectors_of_an_uncompressed_cd_chd() {
    let dir = tempfile::tempdir().unwrap();
    let mut sectors = iso_sectors("TEST", None, &[]);
    sectors[9][..4].copy_from_slice(b"NINE");
    let chd = write(
        dir.path(),
        "disc.chd",
        &to_chd(&to_raw(&sectors, 2), "MODE2_RAW"),
    );
    let mut reader = open_data_track(&chd).unwrap();
    assert_eq!(&reader.read_user_data(9).unwrap()[..4], b"NINE");
    assert_eq!(&reader.read_user_data(16).unwrap()[1..6], b"CD001");
    assert!(reader.read_user_data(sectors.len() as u32).is_err());
}

#[test]
fn non_chd_file_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "fake.chd", b"not a chd at all, just text");
    assert!(open_data_track(&path).is_err());
}

#[test]
fn gd_rom_chd_opens_the_high_density_track_after_padding() {
    use crate::chd_gd_tracks::boot_track;
    use crate::test_gd_images::gd_chd;
    let dir = tempfile::tempdir().unwrap();
    let path = crate::test_images::write(dir.path(), "g.chd", &gd_chd("NSTY-00001"));
    // Track 1: 5 frames → padded to 8; track 2: 3 → 4; track 3 starts at frame 12.
    let mut reader = ChdTrackReader::open_track(&path, boot_track).unwrap();
    assert_eq!(reader.first_frame, 12);
    assert!(reader
        .read_user_data(0)
        .unwrap()
        .starts_with(b"SEGA SEGAKATANA"));
    let mut track1 = ChdTrackReader::open(&path).unwrap();
    assert_eq!(track1.first_frame, 0);
    assert!(!track1.read_user_data(0).unwrap().starts_with(b"SEGA"));
}
