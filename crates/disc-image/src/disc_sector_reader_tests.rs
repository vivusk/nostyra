//! Sector reader tests: layout detection, cue modes, bounds.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::test_images::{iso_sectors, to_iso, to_raw, write};

fn sample() -> Vec<Vec<u8>> {
    let mut sectors = iso_sectors("TEST", None, &[]);
    sectors[5][..4].copy_from_slice(b"FIVE");
    sectors
}

#[test]
fn reads_raw_mode2_mode1_and_iso_layouts() {
    let dir = tempfile::tempdir().unwrap();
    let sectors = sample();
    for (name, bytes) in [
        ("m2.bin", to_raw(&sectors, 2)),
        ("m1.bin", to_raw(&sectors, 1)),
        ("plain.iso", to_iso(&sectors)),
    ] {
        let mut reader = open_data_track(&write(dir.path(), name, &bytes)).unwrap();
        assert_eq!(&reader.read_user_data(5).unwrap()[..4], b"FIVE", "{name}");
        assert_eq!(
            &reader.read_user_data(16).unwrap()[1..6],
            b"CD001",
            "{name}"
        );
    }
}

#[test]
fn cue_selects_first_data_track_and_its_mode() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "audio.bin", &[0u8; 2352 * 4]);
    write(dir.path(), "data.bin", &to_raw(&sample(), 2));
    let cue = write(
        dir.path(),
        "game.cue",
        b"FILE \"audio.bin\" BINARY\nTRACK 01 AUDIO\nFILE \"data.bin\" BINARY\nTRACK 02 MODE2/2352\nINDEX 01 00:00:00\n",
    );
    let mut reader = open_data_track(&cue).unwrap();
    assert_eq!(&reader.read_user_data(5).unwrap()[..4], b"FIVE");
}

#[test]
fn missing_cue_data_file_names_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let cue = write(
        dir.path(),
        "x.cue",
        b"FILE \"Gone.bin\" BINARY\nTRACK 01 MODE2/2352\n",
    );
    let err = open_data_track(&cue).err().unwrap();
    assert_eq!(err.to_string(), "track file 'Gone.bin' not found");
}

#[test]
fn short_or_out_of_range_reads_error_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    let mut tiny = open_data_track(&write(dir.path(), "tiny.bin", &[1, 2, 3])).unwrap();
    assert!(matches!(
        tiny.read_user_data(0),
        Err(DiscError::SectorOutOfRange(0))
    ));
    let mut reader = open_data_track(&write(dir.path(), "s.iso", &to_iso(&sample()))).unwrap();
    assert!(matches!(
        reader.read_user_data(10_000),
        Err(DiscError::SectorOutOfRange(10_000))
    ));
}

#[test]
fn cue_index01_skips_a_stored_pregap_and_sheets_are_not_images() {
    let dir = tempfile::tempdir().unwrap();
    let mut image = vec![0u8; 2352 * 150]; // 2 s pregap stored in the data file
    image.extend(to_raw(&sample(), 2));
    write(dir.path(), "data.bin", &image);
    let cue = write(
        dir.path(),
        "pregap.cue",
        b"FILE \"data.bin\" BINARY\nTRACK 01 MODE2/2352\nINDEX 00 00:00:00\nINDEX 01 00:02:00\n",
    );
    let mut reader = open_data_track(&cue).unwrap();
    assert_eq!(&reader.read_user_data(5).unwrap()[..4], b"FIVE");
    for sheet in ["x.m3u", "EBOOT.PBP"] {
        let path = write(dir.path(), sheet, b"whatever");
        assert!(
            matches!(open_data_track(&path), Err(DiscError::Format(_))),
            "{sheet}"
        );
    }
}
