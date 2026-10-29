//! GameCube / Wii probe arms through [`crate::DiscImageProbe`]: claims by header
//! magic, never by extension; `.ciso` vs PSP `.cso` dispatch; per-extension reject
//! texts; the GameCube playlist rule.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use system_catalog::DiscProbe;

use super::*;
use crate::test_images::{iso_sectors, ps1_sectors, to_iso, to_raw, write, US_LICENCE};
use crate::test_nintendo_images::{ciso, disc_header, dol, plain, wbfs, wia, BLR, MTSPR_HID4_R0};
use crate::test_psp_containers::cso;
use crate::test_psp_images::psp_iso_sectors;
use crate::DiscImageProbe;

const GC: DiscSignature = DiscSignature::GameCube;
const WII: DiscSignature = DiscSignature::Wii;

fn probe(signature: DiscSignature, path: &Path) -> Result<(), String> {
    DiscImageProbe.check(signature, path)
}

fn err(text: &str) -> Result<(), String> {
    Err(text.to_owned())
}

#[test]
fn iso_is_claimed_by_its_header_magic_only() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let gc = write(
        d,
        "gc.iso",
        &plain(
            &disc_header(NintendoDiscKind::GameCube, "GALE01", 0, 0),
            0x400,
        ),
    );
    let wii = write(
        d,
        "wii.iso",
        &plain(&disc_header(NintendoDiscKind::Wii, "RJ2E01", 0, 0), 0x400),
    );
    assert_eq!(probe(GC, &gc), Ok(()));
    assert_eq!(probe(WII, &gc), err(NOT_ANY_ISO));
    assert_eq!(probe(WII, &wii), Ok(()));
    assert_eq!(probe(GC, &wii), err(NOT_GAMECUBE));
    let ps2 = write(
        d,
        "ps2.iso",
        &to_iso(&iso_sectors("PLAYSTATION", Some(US_LICENCE), &[])),
    );
    let psp = write(d, "psp.iso", &to_iso(&psp_iso_sectors()));
    for foreign in [&ps2, &psp] {
        assert_eq!(probe(GC, foreign), err(NOT_GAMECUBE));
        assert_eq!(probe(WII, foreign), err(NOT_ANY_ISO));
    }
    assert_eq!(
        probe(DiscSignature::Psp, &psp),
        Ok(()),
        "PSP probe unchanged"
    );
}

#[test]
fn ciso_and_psp_cso_are_dispatched_by_extension() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let gc = ciso(
        &disc_header(NintendoDiscKind::GameCube, "GALE01", 0, 0),
        true,
    );
    assert_eq!(probe(GC, &write(d, "g.ciso", &gc)), Ok(()));
    assert!(probe(DiscSignature::Psp, &write(d, "g.cso", &gc)).is_err());
    let psp = cso(&to_iso(&psp_iso_sectors()), 2048, 0);
    assert_eq!(probe(DiscSignature::Psp, &write(d, "p.cso", &psp)), Ok(()));
    assert_eq!(
        probe(GC, &write(d, "p.ciso", &psp)),
        err("damaged CISO header"),
        "a PSP CSO renamed to .ciso is a recognised but foreign CISO"
    );
}

#[test]
fn wii_reject_text_depends_on_the_extension() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let gc = disc_header(NintendoDiscKind::GameCube, "GALE01", 0, 0);
    assert_eq!(probe(WII, &write(d, "g.wbfs", &wbfs(&gc, 9))), err(NOT_WII));
    assert_eq!(
        probe(WII, &write(d, "g.rvz", &wia(&gc, true))),
        err(NOT_GAMECUBE_OR_WII)
    );
    assert_eq!(
        probe(WII, &write(d, "junk.wia", &[0u8; 0x300])),
        err(NOT_GAMECUBE_OR_WII)
    );
    assert_eq!(
        probe(GC, &write(d, "junk.gcm", &[0u8; 0x300])),
        err(NOT_GAMECUBE)
    );
    let mut damaged = gc;
    damaged[0] = b'?';
    let damaged = write(d, "d.iso", &plain(&damaged, 0x400));
    assert_eq!(probe(GC, &damaged), err("damaged disc header"));
    assert_eq!(probe(WII, &damaged), err("damaged disc header"));
}

#[test]
fn executables_follow_the_hid4_rule() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let gc = write(d, "gc.dol", &dol(&[BLR; 4], &[]));
    let wii = write(d, "wii.dol", &dol(&[MTSPR_HID4_R0, BLR], &[]));
    assert_eq!(probe(GC, &gc), Ok(()));
    assert_eq!(probe(WII, &gc), err(NOT_GAMECUBE_OR_WII));
    assert_eq!(probe(WII, &wii), Ok(()));
    assert_eq!(probe(GC, &wii), err(NOT_GAMECUBE));
    assert_eq!(
        probe(GC, &write(d, "x.elf", b"\x7FELF junk")),
        err(NOT_GAMECUBE)
    );
}

#[test]
fn gamecube_playlist_checks_its_first_disc() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let header = disc_header(NintendoDiscKind::GameCube, "GALE01", 0, 0);
    write(d, "d1.iso", &plain(&header, 0x400));
    write(d, "d2.rvz", &wia(&header, true));
    assert_eq!(probe(GC, &write(d, "gc.m3u", b"d1.iso\nd2.rvz\n")), Ok(()));
    // A broken PS1 playlist row (GameCube = last `.m3u` candidate) names all three.
    write(
        d,
        "p.bin",
        &to_raw(&ps1_sectors(None, "BOOT2 = cdrom0:\\X;1"), 2),
    );
    let cue = b"FILE \"p.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
    write(d, "p.cue", cue);
    let ps = write(d, "ps.m3u", b"p.cue\n");
    assert_eq!(probe(GC, &ps), err(NOT_GAMECUBE_PLAYLIST));
    assert_eq!(
        probe(DiscSignature::PlayStation, &ps),
        err("not a PlayStation disc")
    );
    // Unreadable entries keep the shared text.
    let missing = write(d, "missing.m3u", b"gone.iso\n");
    assert_eq!(
        probe(GC, &missing),
        err("disc 'gone.iso' not found — fix the playlist line or rename the file")
    );
    write(
        d,
        "broken.cue",
        b"FILE \"lost.bin\" BINARY\n  TRACK 01 MODE2/2352\n",
    );
    let broken = write(d, "broken.m3u", b"broken.cue\n");
    assert!(probe(GC, &broken)
        .unwrap_err()
        .contains("track file 'lost.bin' not found"));
}
