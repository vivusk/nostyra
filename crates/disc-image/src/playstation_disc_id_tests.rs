//! PS1 id tests: licence check (PS2 rejected), serial forms, region, bounded I/O,
//! plus an ignored dev check over the developer's own discs.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::disc_sector_reader::open_data_track;
use crate::test_images::{
    iso_sectors, ps1_sectors, to_raw, write, CountingReader, EU_LICENCE, JP_LICENCE, US_LICENCE,
};

#[test]
fn normalises_serial_forms() {
    assert_eq!(
        normalize_serial("SLUS_012.34").as_deref(),
        Some("SLUS-01234")
    );
    assert_eq!(
        normalize_serial("SCUS_944.26").as_deref(),
        Some("SCUS-94426")
    );
    assert_eq!(
        normalize_serial("NSTY_000.01").as_deref(),
        Some("NSTY-00001")
    );
    assert_eq!(normalize_serial("SLUS00594").as_deref(), Some("SLUS-00594"));
    assert_eq!(
        normalize_serial("slps-03177").as_deref(),
        Some("SLPS-03177")
    );
    assert_eq!(normalize_serial("PSX.EXE"), None);
    assert_eq!(normalize_serial("12345"), None);
}

#[test]
fn boot_line_variants() {
    assert_eq!(
        boot_serial("BOOT = cdrom:\\SLUS_012.34;1\r\n").as_deref(),
        Some("SLUS-01234")
    );
    assert_eq!(
        boot_serial("BOOT=cdrom:SCES_000.01;1").as_deref(),
        Some("SCES-00001")
    );
    assert_eq!(
        boot_serial("  BOOT = cdrom:\\GAME\\SLPS_031.77;1").as_deref(),
        Some("SLPS-03177")
    );
    assert_eq!(boot_serial("BOOT2 = cdrom0:\\SLUS_200.62;1"), None);
    assert_eq!(boot_serial("TCB = 4"), None);
    assert_eq!(
        boot_serial("boot = cdrom:\\slus_000.01;1").as_deref(),
        Some("SLUS-00001")
    );
}

#[test]
fn recognises_ps1_disc_serial_and_region() {
    let dir = tempfile::tempdir().unwrap();
    for (licence, want) in [
        (US_LICENCE, DiscRegion::America),
        (JP_LICENCE, DiscRegion::Japan),
        (EU_LICENCE, DiscRegion::Europe),
    ] {
        let image = to_raw(
            &ps1_sectors(Some(licence), "BOOT = cdrom:\\SLUS_012.34;1"),
            2,
        );
        let mut reader = open_data_track(&write(dir.path(), "g.bin", &image)).unwrap();
        assert!(is_playstation(reader.as_mut()).unwrap());
        assert_eq!(region(reader.as_mut()).unwrap(), Some(want));
        assert_eq!(
            serial(reader.as_mut()).unwrap().as_deref(),
            Some("SLUS-01234")
        );
    }
}

#[test]
fn ps2_like_disc_without_licence_is_not_ps1() {
    let dir = tempfile::tempdir().unwrap();
    let image = to_raw(&ps1_sectors(None, "BOOT2 = cdrom0:\\SLUS_200.62;1"), 2);
    let mut reader = open_data_track(&write(dir.path(), "ps2.bin", &image)).unwrap();
    assert!(!is_playstation(reader.as_mut()).unwrap());
    assert_eq!(region(reader.as_mut()).unwrap(), None);
    let other = to_raw(&iso_sectors("WIN32", Some(US_LICENCE), &[]), 1);
    let mut reader = open_data_track(&write(dir.path(), "pc.bin", &other)).unwrap();
    assert!(!is_playstation(reader.as_mut()).unwrap());
}

#[test]
fn psx_exe_disc_has_no_serial() {
    let dir = tempfile::tempdir().unwrap();
    let sectors = iso_sectors("PLAYSTATION", Some(US_LICENCE), &[("PSX.EXE", &[1; 100])]);
    let mut reader = open_data_track(&write(dir.path(), "e.bin", &to_raw(&sectors, 2))).unwrap();
    assert!(is_playstation(reader.as_mut()).unwrap());
    assert_eq!(serial(reader.as_mut()).unwrap(), None);
}

#[test]
fn identification_reads_at_most_64_kib() {
    let dir = tempfile::tempdir().unwrap();
    let image = to_raw(
        &ps1_sectors(Some(US_LICENCE), "BOOT = cdrom:\\SLUS_012.34;1"),
        2,
    );
    let mut file = open_data_track(&write(dir.path(), "g.bin", &image)).unwrap();
    let mut counting = CountingReader {
        inner: file.as_mut(),
        bytes: 0,
    };
    is_playstation(&mut counting).unwrap();
    serial(&mut counting).unwrap();
    region(&mut counting).unwrap();
    assert!(counting.bytes <= 64 * 1024, "{} bytes", counting.bytes);
}

/// Dev-only: `cargo test -p disc-image -- --ignored --nocapture` with the developer's
/// discs in `E:/PSX` (read-only).
#[test]
#[ignore = "needs the developer's own PS1 discs in E:/PSX"]
fn developer_discs_identify() {
    for (file, want) in [
        ("E:/PSX/CTR - Crash Team Racing (US).bin", "SCUS-94426"),
        ("E:/PSX/Brigandine - Grand Edition 1 (JP).bin", "SLPS-02661"),
        ("E:/PSX/Emblem Saga (VN).bin", "SLPS-03177"),
    ] {
        let mut file_reader = open_data_track(std::path::Path::new(file)).unwrap();
        let mut reader = CountingReader {
            inner: file_reader.as_mut(),
            bytes: 0,
        };
        assert!(is_playstation(&mut reader).unwrap(), "{file}");
        let got = serial(&mut reader).unwrap();
        let reg = region(&mut reader).unwrap();
        println!("{file}: {got:?} {reg:?} read {} bytes", reader.bytes);
        assert_eq!(got.as_deref(), Some(want));
    }
}
