//! Classifier with the real disc probe on GameCube / Wii files: each container goes
//! to the system its header magic names, `.dol` splits on the Wii-only HID4 word, a
//! PS2-like or PSP `.iso` is never claimed by GC / Wii (and the PSP one still goes
//! to PSP, a UMD Video keeps its own reason), another CPU's `.elf` is a quiet
//! `NotRom`, a GC `.m3u` follows its first disc. Lives here (not in system-catalog)
//! because the probe is `disc_image::DiscImageProbe`, which depends on the registry.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use disc_image::test_images::{iso_sectors, to_iso, write};
use disc_image::test_nintendo_images::{ciso, disc_header, dol, elf, gcz, plain, wbfs, wia};
use disc_image::test_nintendo_images::{BLR, MTSPR_HID4_R0};
use disc_image::test_psp_images::{iso_tree, psp_iso_sectors};
use disc_image::DiscImageProbe;
use disc_image::NintendoDiscKind::{self, GameCube, Wii};
use system_catalog::{classify_path, ClassifyError};

fn system_of(path: &Path) -> Result<&'static str, ClassifyError> {
    classify_path(path, &DiscImageProbe).map(|spec| spec.id)
}

fn header(kind: NintendoDiscKind) -> [u8; 0x80] {
    disc_header(kind, if kind == Wii { "RSPE01" } else { "GALE01" }, 0, 0)
}

#[test]
fn every_container_is_classified_by_its_header_magic() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    for kind in [GameCube, Wii] {
        let want = if kind == Wii { "wii" } else { "gc" };
        let h = header(kind);
        for (name, bytes) in [
            ("a.iso", plain(&h, 0x8000)),
            ("a.ciso", ciso(&h, true)),
            ("a.gcz", gcz(&h, 2, false)),
            ("a.wia", wia(&h, false)),
            ("a.rvz", wia(&h, true)),
        ] {
            let path = write(d, &format!("{want}-{name}"), &bytes);
            assert_eq!(system_of(&path), Ok(want), "{name}");
        }
    }
    let gcm = write(d, "a.gcm", &plain(&header(GameCube), 0x8000));
    assert_eq!(system_of(&gcm), Ok("gc"));
    let wbfs_file = write(d, "a.wbfs", &wbfs(&header(Wii), 9));
    assert_eq!(system_of(&wbfs_file), Ok("wii"));
    // `.gcm` is GC-only: a Wii header in it is rejected with the GC text.
    let wii_gcm = write(d, "w.gcm", &plain(&header(Wii), 0x8000));
    assert_eq!(
        system_of(&wii_gcm),
        Err(ClassifyError::DiscRejected("not a GameCube disc".into()))
    );
}

#[test]
fn executables_split_on_hid4_and_foreign_isos_are_not_claimed() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let gc_dol = write(d, "gc.dol", &dol(&[BLR], &[]));
    let wii_dol = write(d, "wii.dol", &dol(&[MTSPR_HID4_R0, BLR], &[]));
    let gc_elf = write(d, "gc.elf", &elf(20, &[BLR]));
    let wii_elf = write(d, "wii.elf", &elf(20, &[MTSPR_HID4_R0]));
    assert_eq!(system_of(&gc_dol), Ok("gc"));
    assert_eq!(system_of(&wii_dol), Ok("wii"));
    assert_eq!(system_of(&gc_elf), Ok("gc"));
    assert_eq!(system_of(&wii_elf), Ok("wii"));
    // Another CPU's ELF is no GC / Wii file at all: quiet (no scan-problem row).
    let x86 = write(d, "x86.elf", &elf(3, &[BLR]));
    let not_ppc = Err(ClassifyError::NotRom("not a PowerPC executable"));
    assert_eq!(system_of(&x86), not_ppc);
    let roms = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms");
    assert_eq!(system_of(&roms.join("nostyra-test-psp.elf")), not_ppc);
    assert_eq!(system_of(&roms.join("nostyra-test-dc.elf")), not_ppc);
    // A broken PowerPC ELF (no program headers) is still a reported problem.
    let mut broken = elf(20, &[BLR]);
    broken[0x2C..0x2E].copy_from_slice(&[0, 0]);
    let broken = write(d, "broken.elf", &broken);
    assert_eq!(
        system_of(&broken),
        Err(ClassifyError::DiscRejected(
            "not a GameCube or Wii disc".into()
        ))
    );
    let ps2 = write(
        d,
        "ps2.iso",
        // PS2-like: PLAYSTATION volume without the PS1 licence text.
        &to_iso(&iso_sectors("PLAYSTATION", None, &[])),
    );
    assert_eq!(
        system_of(&ps2),
        Err(ClassifyError::DiscRejected(
            "not a PlayStation, PSP, GameCube or Wii disc".into()
        ))
    );
    let psp = write(d, "psp.iso", &to_iso(&psp_iso_sectors()));
    assert_eq!(system_of(&psp), Ok("psp"));
    // PSP's specific reason survives the later GC / Wii "not a … disc".
    let movie = iso_tree("UMD VIDEO", &[("UMD_VIDEO/X.BIN", b"x")]);
    let movie = write(d, "movie.iso", &to_iso(&movie));
    let reason = system_of(&movie).unwrap_err().to_string();
    assert!(reason.contains("UMD Video/Audio disc"), "{reason}");
}

#[test]
fn a_gamecube_playlist_follows_its_first_disc() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    write(d, "Disc 1.rvz", &wia(&header(GameCube), true));
    write(
        d,
        "Disc 2.iso",
        &plain(&disc_header(GameCube, "GALE01", 1, 0), 0x8000),
    );
    let m3u = write(d, "Set.m3u", b"Disc 1.rvz\nDisc 2.iso\n");
    assert_eq!(system_of(&m3u), Ok("gc"));
    write(d, "Foreign.iso", &vec![0u8; 0x9000]);
    let foreign = write(d, "Foreign.m3u", b"# one disc\nForeign.iso\n");
    assert_eq!(
        system_of(&foreign),
        Err(ClassifyError::DiscRejected(
            "not a PlayStation, Dreamcast or GameCube playlist".into()
        ))
    );
}
