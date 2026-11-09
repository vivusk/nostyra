//! PS2 content tags: same executable bytes → one tag across containers; another
//! revision of the same serial → another tag; playlists apart; nothing to tag → None.

use std::path::{Path, PathBuf};

use disc_image::test_images::{to_chd, to_iso, to_raw, write};
use disc_image::test_ps2_images::{ps2_cnf, ps2_volume, zso, TEST_EXE};
use disc_image::test_psp_containers::{cso, dvd_chd};
use system_catalog::{PS2, PSX};

use crate::state_content_tag;

/// A `PLAYSTATION` volume booting `exe` (an ELF-shaped file of `body` bytes).
fn sectors(exe: &str, body: &[u8]) -> Vec<Vec<u8>> {
    let cnf = ps2_cnf(exe);
    ps2_volume(
        "NOSTYRA_PS2",
        &[("SYSTEM.CNF", cnf.as_bytes()), (exe, body)],
    )
}

fn iso(dir: &Path, name: &str, exe: &str, body: &[u8]) -> PathBuf {
    write(dir, name, &to_iso(&sectors(exe, body)))
}

fn tag(path: &Path) -> Option<String> {
    state_content_tag(&PS2, path)
}

#[test]
fn every_container_of_one_disc_shares_the_tag_another_revision_does_not() {
    let temp = tempfile::tempdir().unwrap();
    let d = temp.path();
    let rev0 = vec![0x11u8; 3000];
    let plain = to_iso(&sectors(TEST_EXE, &rev0));
    let raw = to_raw(&sectors(TEST_EXE, &rev0), 2);
    write(d, "g.bin", &raw);
    let cue = b"FILE \"g.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
    let files = [
        write(d, "g.iso", &plain),
        write(d, "g.cue", cue),
        write(d, "dvd.chd", &dvd_chd(&plain, true)),
        write(d, "cd.chd", &to_chd(&raw, "MODE2_RAW")),
        write(d, "g.cso", &cso(&plain, 2048, 0)),
        write(d, "g.zso", &zso(&plain, 2048, 1)),
    ];
    let want = tag(&files[0]).expect("tagged");
    let (exe, crc) = want.split_once(':').unwrap();
    assert_eq!(exe, format!("{TEST_EXE};1"));
    assert_eq!(crc, format!("{:08X}", crc32fast::hash(&rev0)));
    for file in &files[1..] {
        assert_eq!(
            tag(file).as_deref(),
            Some(want.as_str()),
            "{}",
            file.display()
        );
    }
    // Same serial, one byte of the executable changed (Rev 1 / hack): another tag.
    let mut rev1 = rev0.clone();
    rev1[100] ^= 1;
    let other = iso(d, "rev1.iso", TEST_EXE, &rev1);
    assert_ne!(tag(&other), Some(want.clone()));
    assert!(tag(&other).unwrap().starts_with(&format!("{TEST_EXE};1:")));
}

#[test]
fn executable_in_a_subfolder_and_only_its_first_64_kib_count() {
    let temp = tempfile::tempdir().unwrap();
    let d = temp.path();
    let body = vec![0x42u8; 70 * 1024];
    let mut tail_changed = body.clone();
    tail_changed[69 * 1024] ^= 1;
    let a = iso(d, "a.iso", "BOOT.ELF", &body);
    let b = iso(d, "b.iso", "BOOT.ELF", &tail_changed);
    assert_eq!(tag(&a), tag(&b), "bytes past 64 KiB are not read");
    assert!(tag(&a).unwrap().starts_with("BOOT.ELF;1:"));
    // `BOOT2 = cdrom0:\DEMO\RUN.ELF;1`: the path is walked from the root.
    let cnf = b"BOOT2 = cdrom0:\\DEMO\\RUN.ELF;1\r\nVMODE = NTSC\r\n";
    let mut sectors = disc_image::test_images::iso_sectors(
        "PLAYSTATION",
        None,
        &[("SYSTEM.CNF", cnf), ("DEMO", b"")],
    );
    sectors[16][40..72].copy_from_slice(format!("{:<32}", "HB").as_bytes());
    let nested = write(d, "nested.iso", &to_iso(&sectors));
    // The fixture builder has no subdirectories: a missing executable is untagged.
    assert_eq!(tag(&nested), None);
}

#[test]
fn playlist_is_tagged_apart_from_its_first_disc() {
    let temp = tempfile::tempdir().unwrap();
    let d = temp.path();
    let disc1 = iso(d, "d1.iso", "SLUS_203.12", &[1; 600]);
    iso(d, "d2.iso", "SLUS_203.13", &[2; 600]);
    let m3u = write(d, "set.m3u", b"d1.iso\r\nd2.iso\r\n");
    let first = tag(&disc1).unwrap();
    assert_eq!(tag(&m3u), Some(format!("m3u:{first}")));
    assert_eq!(tag(&write(d, "empty.m3u", b"\n")), None);
}

#[test]
fn other_systems_missing_boot2_and_unreadable_images_are_untagged() {
    let temp = tempfile::tempdir().unwrap();
    let d = temp.path();
    let ps1 = write(
        d,
        "ps1.bin",
        &to_raw(
            &disc_image::test_images::ps1_sectors(
                Some(disc_image::test_images::US_LICENCE),
                r"BOOT = cdrom:\SLUS_012.34;1",
            ),
            2,
        ),
    );
    assert_eq!(state_content_tag(&PSX, &ps1), None, "PS1: untagged system");
    assert_eq!(tag(&ps1), None, "no BOOT2 line");
    let no_cnf = disc_image::test_images::iso_sectors("PLAYSTATION", None, &[]);
    assert_eq!(tag(&write(d, "nocnf.iso", &to_iso(&no_cnf))), None);
    assert_eq!(tag(&write(d, "junk.iso", &[0; 4096])), None);
    assert_eq!(tag(&d.join("gone.iso")), None);
}
