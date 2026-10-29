//! PBP tests: synthetic header + PARAM.SFO + PSAR; PS1 classic vs PSP eboot.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::test_images::write;

fn sfo(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut keys: Vec<u8> = Vec::new();
    let mut data: Vec<u8> = Vec::new();
    let mut table: Vec<u8> = Vec::new();
    for (key, value) in entries {
        let key_off = keys.len() as u16;
        keys.extend(key.as_bytes());
        keys.push(0);
        let data_off = data.len() as u32;
        let mut v = value.as_bytes().to_vec();
        v.push(0);
        let len = v.len() as u32;
        data.extend(&v);
        table.extend(key_off.to_le_bytes());
        table.extend(0x0204u16.to_le_bytes());
        table.extend(len.to_le_bytes());
        table.extend(len.to_le_bytes());
        table.extend(data_off.to_le_bytes());
    }
    let key_start = 20 + table.len();
    let data_start = key_start + keys.len();
    let mut out = b"\0PSF".to_vec();
    out.extend(0x0101u32.to_le_bytes());
    out.extend((key_start as u32).to_le_bytes());
    out.extend((data_start as u32).to_le_bytes());
    out.extend((entries.len() as u32).to_le_bytes());
    out.extend(table);
    out.extend(keys);
    out.extend(data);
    out
}

fn pbp(sfo: &[u8]) -> Vec<u8> {
    let mut out = b"\0PBP".to_vec();
    out.extend(0x0001_0000u32.to_le_bytes());
    out.extend(0x28u32.to_le_bytes());
    out.extend((0x28 + sfo.len() as u32).to_le_bytes());
    out.resize(0x28, 0);
    out.extend(sfo);
    out.extend([0u8; 64]);
    out
}

#[test]
fn ps1_classic_gives_category_and_serial() {
    let dir = tempfile::tempdir().unwrap();
    let body = pbp(&sfo(&[
        ("CATEGORY", "ME"),
        ("DISC_ID", "SLUS00594"),
        ("TITLE", "Game"),
    ]));
    let info = PbpInfo::read(&write(dir.path(), "EBOOT.PBP", &body)).unwrap();
    assert!(info.is_playstation());
    assert_eq!(info.serial().as_deref(), Some("SLUS-00594"));
}

#[test]
fn psp_eboot_is_not_ps1() {
    let dir = tempfile::tempdir().unwrap();
    let body = pbp(&sfo(&[("CATEGORY", "UG"), ("DISC_ID", "ULUS10041")]));
    let info = PbpInfo::read(&write(dir.path(), "psp.pbp", &body)).unwrap();
    assert!(!info.is_playstation());
}

#[test]
fn bad_headers_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(PbpInfo::read(&write(dir.path(), "a.pbp", b"\0PBP")).is_err());
    assert!(PbpInfo::read(&write(dir.path(), "b.pbp", &[0u8; 64])).is_err());
    let mut broken = pbp(&sfo(&[("CATEGORY", "ME")]));
    broken[0x0C..0x10].copy_from_slice(&0x10u32.to_le_bytes()); // end before start
    assert!(PbpInfo::read(&write(dir.path(), "c.pbp", &broken)).is_err());
}

#[test]
fn kind_follows_category_and_psar_magic() {
    use crate::test_psp_containers::pbp as eboot;
    use crate::test_psp_images::game_sfo;
    let dir = tempfile::tempdir().unwrap();
    let kind = |name: &str, category: &str, psar: &[u8]| {
        let path = write(dir.path(), name, &eboot(&game_sfo(category), psar));
        PbpInfo::read(&path).unwrap()
    };
    let ps1 = kind("ps1.pbp", "ME", b"PSISOIMG0000");
    assert_eq!(ps1.kind, PbpKind::Ps1);
    assert!(ps1.is_playstation());
    let odd = kind("odd.pbp", "EG", b"PSTITLEIMG000000");
    assert_eq!(odd.kind, PbpKind::Ps1, "PS1 data, PSP category");
    assert!(!odd.is_playstation(), "claimed by neither system");
    let psn = kind("psn.pbp", "EG", b"NPUMDIMG");
    assert_eq!(psn.kind, PbpKind::Psp);
    assert_eq!(psn.sfo.title.as_deref(), Some("Nostyra Test PSP"));
    assert_eq!(kind("hb.pbp", "MG", b"").kind, PbpKind::Psp, "empty PSAR");
}
