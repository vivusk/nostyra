//! PS2 probe tests over every container, the negatives (PS1, PSP, data disc) and the
//! per-extension reject texts; `.zso` reaches `CsoReader` at both dispatch sites.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use super::*;
use crate::disc_track_access::{DiscTrack, TrackSelector};
use crate::test_images::{iso_sectors, ps1_sectors, to_chd, to_iso, to_raw, write, US_LICENCE};
use crate::test_ps2_images::{ps2_sectors, ps2_volume, zso, TEST_EXE};
use crate::test_psp_containers::{cso, dvd_chd};
use crate::test_psp_images::psp_iso_sectors;
use crate::DiscImageProbe;
use system_catalog::{DiscProbe, DiscSignature};

fn probe(path: &Path) -> Result<(), String> {
    DiscImageProbe.check(DiscSignature::PlayStation2, path)
}

fn err(text: &str) -> Result<(), String> {
    Err(text.to_owned())
}

#[test]
fn accepts_ps2_discs_in_every_container() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let sectors = ps2_sectors(TEST_EXE);
    let iso = to_iso(&sectors);
    let raw = to_raw(&sectors, 2);
    assert_eq!(probe(&write(d, "g.iso", &iso)), Ok(()));
    assert_eq!(probe(&write(d, "g.bin", &raw)), Ok(()));
    assert_eq!(probe(&write(d, "dvd.chd", &dvd_chd(&iso, true))), Ok(()));
    assert_eq!(
        probe(&write(d, "cd.chd", &to_chd(&raw, "MODE2_RAW"))),
        Ok(())
    );
    assert_eq!(probe(&write(d, "g.cso", &cso(&iso, 2048, 0))), Ok(()));
    assert_eq!(probe(&write(d, "g.zso", &zso(&iso, 2048, 0))), Ok(()));
    let cue = b"FILE \"g.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
    assert_eq!(probe(&write(d, "g.cue", cue)), Ok(()));
    assert_eq!(probe(&write(d, "set.m3u", b"g.cue\ng.iso\n")), Ok(()));
    // A serial-less disc is still a PS2 disc (identity falls back to the volume).
    let cnf = b"BOOT2 = cdrom0:\\BOOT.ELF;1\r\nVER = 1.00\r\nVMODE = PAL\r\n";
    let homebrew = ps2_volume("HB", &[("SYSTEM.CNF", cnf), ("BOOT.ELF", &[1; 64])]);
    assert_eq!(probe(&write(d, "hb.iso", &to_iso(&homebrew))), Ok(()));
}

#[test]
fn rejects_other_discs_with_the_extension_text() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let ps1 = ps1_sectors(Some(US_LICENCE), "BOOT = cdrom:\\SLUS_012.34;1");
    assert_eq!(probe(&write(d, "ps1.iso", &to_iso(&ps1))), err(NOT_PS2));
    assert_eq!(
        probe(&write(d, "ps1.bin", &to_raw(&ps1, 2))),
        err(NOT_ANY_BIN)
    );
    let psp = to_iso(&psp_iso_sectors());
    assert_eq!(probe(&write(d, "umd.iso", &psp)), err(NOT_PS2));
    assert_eq!(
        probe(&write(d, "umd.cso", &cso(&psp, 2048, 0))),
        err(NOT_PS2)
    );
    let data = to_iso(&iso_sectors("WIN32", None, &[("README.TXT", b"hi")]));
    assert_eq!(probe(&write(d, "pc.iso", &data)), err(NOT_PS2));
    // `PLAYSTATION` volume without `BOOT2`: a PS1 disc that lost its licence text.
    let no_boot2 = to_raw(&ps1_sectors(None, "TCB = 4"), 2);
    assert_eq!(probe(&write(d, "x.bin", &no_boot2)), err(NOT_ANY_BIN));
    assert_eq!(
        probe(&write(d, "audio.bin", &vec![0x55u8; 2352 * 40])),
        err(NOT_ANY_BIN)
    );
    assert_eq!(
        probe(&write(d, "short.bin", b"tiny file here!!")),
        err(NOT_ANY_BIN)
    );
    assert_eq!(probe(&write(d, "e.pbp", b"\0PBP")), err(NOT_PS2));
    assert_eq!(probe(&write(d, "e.cdi", &[0u8; 64])), err(NOT_PS2));
}

#[test]
fn unreadable_containers_keep_their_own_text() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let cue = write(
        d,
        "lost.cue",
        b"FILE \"gone.bin\" BINARY\n  TRACK 01 MODE2/2352\n",
    );
    assert_eq!(
        probe(&cue),
        err("track file 'gone.bin' not found — fix the FILE line or rename the file")
    );
    let mut v2 = cso(&to_iso(&ps2_sectors(TEST_EXE)), 2048, 0);
    v2[20] = 2;
    assert_eq!(
        probe(&write(d, "v2.zso", &v2)),
        err("CSO version 2 is not supported")
    );
    assert_eq!(
        probe(&write(d, "junk.zso", b"not a zso at all, really")),
        err("not a CSO / ZSO file")
    );
    assert_eq!(
        probe(&write(d, "missing.m3u", b"nowhere.iso\n")),
        err("disc 'nowhere.iso' not found — fix the playlist line or rename the file")
    );
}

#[test]
fn zso_opens_through_both_dispatch_sites() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let iso = to_iso(&ps2_sectors(TEST_EXE));
    let iso_path = write(d, "t.iso", &iso);
    let zso_path = write(d, "t.zso", &zso(&iso, 2048, 1));
    let mut plain = open_data_track(&iso_path).unwrap();
    let mut packed = open_data_track(&zso_path).unwrap();
    assert_eq!(plain.sector_count(), packed.sector_count());
    for lba in [0, 16, 18, 19] {
        assert_eq!(
            plain.read_user_data(lba).unwrap(),
            packed.read_user_data(lba).unwrap(),
            "lba {lba}"
        );
    }
    let mut plain = DiscTrack::open(&iso_path, TrackSelector::FirstData).unwrap();
    let mut packed = DiscTrack::open(&zso_path, TrackSelector::FirstData).unwrap();
    assert_eq!(plain.info(), packed.info());
    let (mut a, mut b) = ([0u8; 4096], [0u8; 4096]);
    assert_eq!(plain.read_user_data_abs(16, &mut a), 4096);
    assert_eq!(packed.read_user_data_abs(16, &mut b), 4096);
    assert_eq!(a, b);
    assert_eq!(&a[1..6], b"CD001");
}
