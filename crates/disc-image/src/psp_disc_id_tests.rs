//! PSP disc id tests: the in-code test UMD in every container (`.iso`, `.cso`, DVD
//! `.chd` with/without `DVD ` tag, `EBOOT.PBP`), serial sources, refused volumes.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::disc_sector_reader::{open_data_track, SECTOR_DATA_BYTES};
use crate::pbp_sfo::{PbpInfo, PbpKind};
use crate::test_images::{ps1_sectors, to_chd, to_iso, to_raw, write, US_LICENCE};
use crate::test_psp_containers::{cso, dvd_chd, pbp};
use crate::test_psp_images::{game_sfo, iso_tree, psp_iso_sectors, TEST_TITLE};

fn identity_of(dir: &std::path::Path, name: &str, bytes: &[u8]) -> Result<PspDiscId, DiscError> {
    let mut reader = open_data_track(&write(dir, name, bytes))?;
    read_psp_identity(reader.as_mut())
}

#[test]
fn every_container_of_the_test_umd_gives_serial_and_title() {
    let dir = tempfile::tempdir().unwrap();
    let iso = to_iso(&psp_iso_sectors());
    let images = [
        ("t.iso", iso.clone()),
        ("t.cso", cso(&iso, SECTOR_DATA_BYTES, 0)),
        ("t.chd", dvd_chd(&iso, true)),
        ("untagged.chd", dvd_chd(&iso, false)),
    ];
    for (name, bytes) in images {
        let id = identity_of(dir.path(), name, &bytes).unwrap();
        assert_eq!(id.serial.as_deref(), Some("NSTY-00001"), "{name}");
        assert_eq!(id.title.as_deref(), Some(TEST_TITLE), "{name}");
        assert_eq!(id.disc_version.as_deref(), Some("1.00"), "{name}");
    }
    let eboot = write(dir.path(), "EBOOT.PBP", &pbp(&game_sfo("MG"), b""));
    let info = PbpInfo::read(&eboot).unwrap();
    assert_eq!(info.kind, PbpKind::Psp);
    let id = PspDiscId::from_sfo(&info.sfo);
    assert_eq!(id.serial.as_deref(), Some("NSTY-00001"));
    assert_eq!(id.title.as_deref(), Some(TEST_TITLE));
}

#[test]
fn serial_falls_back_to_param_sfo_and_quoted_system_id_counts() {
    let dir = tempfile::tempdir().unwrap();
    let sfo = game_sfo("UG");
    let sectors = iso_tree("\"PSP GAME\"", &[("PSP_GAME/PARAM.SFO", &sfo)]);
    let id = identity_of(dir.path(), "q.iso", &to_iso(&sectors)).unwrap();
    assert_eq!(id.serial.as_deref(), Some("NSTY-00001"), "from DISC_ID");
    let bare = iso_tree("PSP GAME", &[("UMD_DATA.BIN", b"ULJM-05001|x")]);
    let id = identity_of(dir.path(), "bare.iso", &to_iso(&bare)).unwrap();
    assert_eq!(id.serial.as_deref(), Some("ULJM-05001"));
    assert_eq!(id.title, None, "no PARAM.SFO");
    let dir_only = iso_tree("", &[("PSP_GAME/PARAM.SFO", &sfo)]);
    assert!(identity_of(dir.path(), "d.iso", &to_iso(&dir_only)).is_ok());
}

#[test]
fn other_volumes_are_refused_with_a_reason() {
    let dir = tempfile::tempdir().unwrap();
    let video = iso_tree("UMD VIDEO", &[("UMD_VIDEO/PARAM.SFO", &game_sfo("MV"))]);
    let err = identity_of(dir.path(), "v.iso", &to_iso(&video)).unwrap_err();
    assert_eq!(err.to_string(), "UMD Video/Audio disc");
    let ps1 = ps1_sectors(Some(US_LICENCE), "BOOT = cdrom:\\SLUS_012.34;1");
    let err = identity_of(dir.path(), "ps1.iso", &to_iso(&ps1)).unwrap_err();
    assert!(
        matches!(err, DiscError::NotPsp("not a PSP UMD image")),
        "{err}"
    );
    let ps1_chd = to_chd(&to_raw(&ps1, 2), "MODE2_RAW");
    let err = identity_of(dir.path(), "ps1.chd", &ps1_chd).unwrap_err();
    assert!(
        matches!(err, DiscError::NotPsp(_)),
        "CD CHD read as a CD: {err}"
    );
    let err = identity_of(dir.path(), "z.iso", &vec![0u8; 2048 * 20]).unwrap_err();
    assert!(matches!(err, DiscError::Format(_)));
}
