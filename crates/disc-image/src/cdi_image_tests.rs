//! DiscJuggler parser tests on synthetic images of every supported version.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::disc_sector_reader::DiscReader;
use crate::test_cdi_image::{cdi_image, CDI_V2, CDI_V3, CDI_V35};
use crate::test_images::write;

#[test]
fn parses_sessions_and_tracks_of_every_version() {
    let dir = tempfile::tempdir().unwrap();
    for version in [CDI_V2, CDI_V3, CDI_V35] {
        let path = write(
            dir.path(),
            &format!("{version:x}.cdi"),
            &cdi_image(version, "T-1234N"),
        );
        let image = CdiImage::read(&path).unwrap();
        assert_eq!(image.tracks.len(), 2, "{version:x}");
        let audio = &image.tracks[0];
        assert_eq!((audio.session, audio.mode, audio.sector_size), (0, 0, 2352));
        let boot = image.boot_track().unwrap();
        assert_eq!((boot.session, boot.mode, boot.sector_size), (1, 2, 2336));
        assert_eq!(boot.lba, 11_702, "start LBA + pregap");
        assert_eq!(boot.data_start, 2352 * 4 + 2336 * 2, "after audio + pregap");
        let mut reader = CdiImage::open_track(&path, boot).unwrap();
        assert!(reader
            .read_user_data(0)
            .unwrap()
            .starts_with(b"SEGA SEGAKATANA"));
        assert!(reader.read_user_data(3).is_err(), "length bounds the track");
    }
}

#[test]
fn unknown_versions_and_junk_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut bad_version = cdi_image(CDI_V3, "X");
    let at = bad_version.len() - 8;
    bad_version[at..at + 4].copy_from_slice(&0x8000_0007u32.to_le_bytes());
    let err = CdiImage::read(&write(dir.path(), "v.cdi", &bad_version)).unwrap_err();
    assert_eq!(err.to_string(), "unsupported DiscJuggler version");
    for junk in [&b"tiny"[..], &[0u8; 64], &cdi_image(CDI_V2, "X")[..9000]] {
        assert!(CdiImage::read(&write(dir.path(), "j.cdi", junk)).is_err());
    }
    // Header offset pointing past the table: rejected, not a panic.
    let mut far = cdi_image(CDI_V35, "X");
    let at = far.len() - 4;
    far[at..].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(CdiImage::read(&write(dir.path(), "f.cdi", &far)).is_err());
}
