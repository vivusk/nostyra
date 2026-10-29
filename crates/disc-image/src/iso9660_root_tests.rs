//! ISO9660 tests: PVD fields, root lookup, file content.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::disc_sector_reader::open_data_track;
use crate::test_images::{iso_sectors, to_iso, write};

#[test]
fn reads_pvd_and_finds_root_files_case_insensitively() {
    let dir = tempfile::tempdir().unwrap();
    let body = vec![7u8; 5000];
    let sectors = iso_sectors(
        "PLAYSTATION",
        None,
        &[("README.TXT", b"hi"), ("BIG.DAT", &body)],
    );
    let mut reader = open_data_track(&write(dir.path(), "v.iso", &to_iso(&sectors))).unwrap();
    let pvd = PrimaryVolume::read(reader.as_mut()).unwrap();
    assert_eq!(pvd.system_id, "PLAYSTATION");
    assert_eq!(pvd.volume_id, "NOSTYRA_TEST");
    let big = pvd
        .find_root_file(reader.as_mut(), "big.dat")
        .unwrap()
        .unwrap();
    assert_eq!(big.size, 5000);
    assert_eq!(read_extent(reader.as_mut(), big, 10_000).unwrap(), body);
    assert_eq!(read_extent(reader.as_mut(), big, 100).unwrap().len(), 100);
    assert!(pvd
        .find_root_file(reader.as_mut(), "MISSING.BIN")
        .unwrap()
        .is_none());
}

#[test]
fn non_iso_data_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut reader = open_data_track(&write(dir.path(), "z.iso", &vec![0u8; 2048 * 20])).unwrap();
    assert!(matches!(
        PrimaryVolume::read(reader.as_mut()),
        Err(DiscError::Format(_))
    ));
}

#[test]
fn finds_files_by_path_through_nested_and_multi_sector_dirs() {
    use crate::test_psp_images::iso_tree;
    let dir = tempfile::tempdir().unwrap();
    let names: Vec<String> = (0..80)
        .map(|i| format!("PSP_GAME/USRDIR/FILE{i:03}.DAT"))
        .collect();
    let mut files: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b"x"[..])).collect();
    files.push(("PSP_GAME/SYSDIR/EBOOT.BIN", b"elf"));
    files.push(("PSP_GAME/USRDIR/LAST.BIN", b"last"));
    let image = to_iso(&iso_tree("PSP GAME", &files));
    let mut reader = open_data_track(&write(dir.path(), "t.iso", &image)).unwrap();
    let pvd = PrimaryVolume::read(reader.as_mut()).unwrap();
    let usrdir = pvd
        .find_path(reader.as_mut(), &["psp_game", "USRDIR"])
        .unwrap()
        .unwrap();
    assert!(usrdir.size > 2048, "USRDIR spans several sectors");
    let last = pvd
        .find_path(reader.as_mut(), &["PSP_GAME", "USRDIR", "LAST.BIN"])
        .unwrap()
        .unwrap();
    assert_eq!(read_extent(reader.as_mut(), last, 100).unwrap(), b"last");
    let eboot = pvd
        .find_path(reader.as_mut(), &["PSP_GAME", "SYSDIR", "EBOOT.BIN"])
        .unwrap()
        .unwrap();
    assert_eq!(eboot.size, 3);
    for missing in [
        &["NOPE"][..],
        &["PSP_GAME", "NOPE"],
        &["PSP_GAME", "SYSDIR", "X"],
    ] {
        assert!(pvd.find_path(reader.as_mut(), missing).unwrap().is_none());
    }
}
