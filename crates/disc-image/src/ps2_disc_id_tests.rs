//! PS2 id tests: `BOOT2` parsing, the identity key (serial or volume + exe),
//! `VMODE`, bounded I/O.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::iso9660_root::PrimaryVolume;
use crate::playstation_disc_id::{
    boot2_exe, boot2_serial, disc_video_mode, is_playstation, is_playstation2, system_cnf,
};
use crate::test_images::{
    iso_sectors, ps1_sectors, to_iso, to_raw, write, CountingReader, US_LICENCE,
};
use crate::test_ps2_images::{ps2_sectors, ps2_volume, TEST_EXE, TEST_SERIAL, TEST_VOLUME_ID};
use system_catalog::Ps2VideoMode;

#[test]
fn boot2_line_variants() {
    let serial = |cnf: &str| boot2_serial(cnf);
    assert_eq!(
        serial("BOOT2 = cdrom0:\\SLUS_203.12;1\r\nVER = 1.01\r\n").as_deref(),
        Some("SLUS-20312")
    );
    assert_eq!(
        serial("BOOT2=cdrom0:SLPS_254.97;1").as_deref(),
        Some("SLPS-25497")
    );
    assert_eq!(
        serial("boot2 = CDROM0:\\slus_200.62").as_deref(),
        Some("SLUS-20062")
    );
    assert_eq!(
        serial("  BOOT2 = cdrom0:\\DEMO\\SLUS_200.62;1").as_deref(),
        Some("SLUS-20062")
    );
    assert_eq!(serial("BOOT = cdrom:\\SLUS_012.34;1"), None, "PS1 line");
    assert_eq!(serial("TCB = 4"), None);
    assert_eq!(serial("BOOT2 = cdrom0:\\BOOT.ELF;1"), None);
    assert_eq!(serial("BOOT2 = cdrom0:\\PS2LOGO"), None);
    assert_eq!(
        boot2_exe("BOOT2 = cdrom0:\\BOOT.ELF;1").as_deref(),
        Some("BOOT.ELF;1")
    );
    assert_eq!(
        boot2_exe("BOOT2 = cdrom0:\\demo\\Menu.elf;1").as_deref(),
        Some("DEMO\\MENU.ELF;1")
    );
    assert_eq!(boot2_exe("BOOT2 = cdrom0:\\"), None);
    assert_eq!(boot2_exe("BOOT = cdrom:\\SLUS_012.34;1"), None);
    assert_eq!(
        disc_video_mode("VMODE = NTSC\r\n"),
        Some(Ps2VideoMode::Ntsc)
    );
    assert_eq!(disc_video_mode("vmode=pal"), Some(Ps2VideoMode::Pal));
    assert_eq!(disc_video_mode("BOOT2 = cdrom0:\\X;1"), None);
}

#[test]
fn ps2_identity_key_is_the_serial_or_the_volume_and_exe() {
    let dir = tempfile::tempdir().unwrap();
    let retail = write(dir.path(), "r.iso", &to_iso(&ps2_sectors(TEST_EXE)));
    let mut reader = open_data_track(&retail).unwrap();
    assert!(is_playstation2(reader.as_mut()).unwrap());
    assert!(!is_playstation(reader.as_mut()).unwrap());
    let key = ps2_identity_key(reader.as_mut()).unwrap().unwrap();
    assert_eq!(key, Ps2IdentityKey::Serial(TEST_SERIAL.into()));
    assert_eq!(key.serial(), Some(TEST_SERIAL));
    let cnf = b"BOOT2 = cdrom0:\\BOOT.ELF;1\r\nVMODE = PAL\r\n";
    let homebrew = ps2_volume("NOSTYRA_HB", &[("SYSTEM.CNF", cnf), ("BOOT.ELF", &[1; 32])]);
    let hb = write(dir.path(), "hb.iso", &to_iso(&homebrew));
    let mut reader = open_data_track(&hb).unwrap();
    assert!(is_playstation2(reader.as_mut()).unwrap());
    let key = ps2_identity_key(reader.as_mut()).unwrap().unwrap();
    assert_eq!(
        key,
        Ps2IdentityKey::VolumeExe {
            volume_id: "NOSTYRA_HB".into(),
            exe: "BOOT.ELF;1".into()
        }
    );
    assert_eq!(key.serial(), None);
    let pvd = PrimaryVolume::read(reader.as_mut()).unwrap();
    let text = system_cnf(reader.as_mut(), &pvd).unwrap().unwrap();
    assert_eq!(disc_video_mode(&text), Some(Ps2VideoMode::Pal));
    // No `BOOT2`: not a PS2 disc, no key.
    let ps1 = write(
        dir.path(),
        "ps1.bin",
        &to_raw(
            &ps1_sectors(Some(US_LICENCE), "BOOT = cdrom:\\SLUS_012.34;1"),
            2,
        ),
    );
    let mut reader = open_data_track(&ps1).unwrap();
    assert!(!is_playstation2(reader.as_mut()).unwrap());
    assert_eq!(ps2_identity_key(reader.as_mut()).unwrap(), None);
    let pc = write(
        dir.path(),
        "pc.iso",
        &to_iso(&iso_sectors("WIN32", None, &[])),
    );
    assert!(!is_playstation2(open_data_track(&pc).unwrap().as_mut()).unwrap());
    assert_eq!(TEST_VOLUME_ID, "NOSTYRA_PS2");
}

#[test]
fn ps2_identification_reads_at_most_64_kib() {
    let dir = tempfile::tempdir().unwrap();
    let image = to_raw(&ps2_sectors(TEST_EXE), 2);
    let mut file = open_data_track(&write(dir.path(), "g.bin", &image)).unwrap();
    let mut counting = CountingReader {
        inner: file.as_mut(),
        bytes: 0,
    };
    assert!(is_playstation2(&mut counting).unwrap());
    assert!(ps2_identity_key(&mut counting).unwrap().is_some());
    assert!(counting.bytes <= 64 * 1024, "{} bytes", counting.bytes);
}

/// Dev-only: `cargo test -p disc-image -- --ignored --nocapture` with the developer's
/// disc in `E:/PS2` (read-only, never a fixture).
#[test]
#[ignore = "needs the developer's own PS2 disc in E:/PS2"]
fn developer_disc_identifies() {
    let path = std::path::Path::new("E:/PS2/Berwick Saga.iso");
    let mut file = open_data_track(path).unwrap();
    let mut reader = CountingReader {
        inner: file.as_mut(),
        bytes: 0,
    };
    assert!(is_playstation2(&mut reader).unwrap());
    assert!(!is_playstation(&mut reader).unwrap());
    let key = ps2_identity_key(&mut reader).unwrap().unwrap();
    let pvd = PrimaryVolume::read(&mut reader).unwrap();
    let cnf = system_cnf(&mut reader, &pvd).unwrap().unwrap();
    println!(
        "{key:?} {:?} read {} bytes\n{cnf}",
        disc_video_mode(&cnf),
        reader.bytes
    );
    assert_eq!(key, Ps2IdentityKey::Serial("SLPS-25497".into()));
    assert_eq!(disc_video_mode(&cnf), Some(Ps2VideoMode::Ntsc));
    assert!(reader.bytes <= 64 * 1024);
    assert_eq!(
        system_catalog::DiscProbe::check(
            &crate::DiscImageProbe,
            system_catalog::DiscSignature::PlayStation2,
            path
        ),
        Ok(())
    );
}
