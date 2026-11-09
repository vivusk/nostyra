//! Classifier with the real disc probe on PlayStation 2 files: every container goes
//! to `ps2`, a PS1 `.bin` / `.cue` stays `psx` and a PS2 one never becomes `psx`, PSP /
//! GameCube / Wii / data images are not claimed, an `.m3u` follows its first disc, the
//! unmatched `.bin` text names both PlayStation systems, an Atari-sized `.bin` stays
//! DAT-gated. Lives here (not in system-catalog) because the probe is
//! `disc_image::DiscImageProbe`, which depends on the registry.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use disc_image::test_images::{
    iso_sectors, ps1_sectors, to_chd, to_iso, to_raw, write, US_LICENCE,
};
use disc_image::test_nintendo_images::{disc_header, plain};
use disc_image::test_ps2_images::{ps2_sectors, zso, TEST_EXE};
use disc_image::test_psp_containers::{cso, dvd_chd};
use disc_image::test_psp_images::psp_iso_sectors;
use disc_image::DiscImageProbe;
use disc_image::NintendoDiscKind::{GameCube, Wii};
use game_library::discover_rom_files;
use system_catalog::{classify_path, ClassifyError, ALL_SYSTEMS};

fn system_of(path: &Path) -> Result<&'static str, ClassifyError> {
    classify_path(path, &DiscImageProbe).map(|spec| spec.id)
}

fn rejected(text: &str) -> Result<&'static str, ClassifyError> {
    Err(ClassifyError::DiscRejected(text.into()))
}

const CUE: &[u8] = b"FILE \"g.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";

#[test]
fn every_ps2_container_is_classified_as_ps2() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let sectors = ps2_sectors(TEST_EXE);
    let iso = to_iso(&sectors);
    let raw = to_raw(&sectors, 2);
    write(d, "g.bin", &raw);
    for (name, bytes) in [
        ("g.iso", iso.clone()),
        ("g.bin", raw.clone()),
        ("g.cue", CUE.to_vec()),
        ("dvd.chd", dvd_chd(&iso, true)),
        ("cd.chd", to_chd(&raw, "MODE2_RAW")),
        ("g.cso", cso(&iso, 2048, 0)),
        ("g.zso", zso(&iso, 2048, 1)),
        ("set.m3u", b"#EXTM3U\ng.cue\ng.iso\n".to_vec()),
    ] {
        assert_eq!(system_of(&write(d, name, &bytes)), Ok("ps2"), "{name}");
    }
}

#[test]
fn ps1_discs_stay_psx_and_other_discs_are_not_claimed() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let ps1 = ps1_sectors(Some(US_LICENCE), "BOOT = cdrom:\\SLUS_012.34;1");
    write(d, "g.bin", &to_raw(&ps1, 2));
    assert_eq!(system_of(&d.join("g.bin")), Ok("psx"));
    assert_eq!(system_of(&write(d, "g.cue", CUE)), Ok("psx"));
    assert_eq!(system_of(&write(d, "p1.iso", &to_iso(&ps1))), Ok("psx"));
    assert_eq!(
        system_of(&write(d, "umd.iso", &to_iso(&psp_iso_sectors()))),
        Ok("psp")
    );
    let umd_cso = cso(&to_iso(&psp_iso_sectors()), 2048, 0);
    assert_eq!(system_of(&write(d, "umd.cso", &umd_cso)), Ok("psp"));
    let gc = plain(&disc_header(GameCube, "GALE01", 0, 0), 0x8000);
    assert_eq!(system_of(&write(d, "gc.iso", &gc)), Ok("gc"));
    let wii = plain(&disc_header(Wii, "RSPE01", 0, 0), 0x8000);
    assert_eq!(system_of(&write(d, "wii.iso", &wii)), Ok("wii"));
    let data = to_iso(&iso_sectors("WIN32", None, &[("README.TXT", b"hi")]));
    assert_eq!(
        system_of(&write(d, "pc.iso", &data)),
        rejected("not a PlayStation, PlayStation 2, PSP, GameCube or Wii disc")
    );
    // Lone `.zso`: PS2 is its only candidate.
    assert_eq!(
        system_of(&write(d, "junk.zso", &zso(&data, 2048, 0))),
        rejected("not a PlayStation 2 disc")
    );
}

#[test]
fn unmatched_bins_name_both_playstations_and_atari_sized_ones_stay_dat_gated() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    // A `PLAYSTATION` volume with neither licence text nor `BOOT2`.
    let neither = to_raw(&ps1_sectors(None, ""), 2);
    assert_eq!(
        system_of(&write(d, "neither.bin", &neither)),
        rejected("not a PlayStation or PlayStation 2 disc")
    );
    assert_eq!(
        system_of(&write(d, "audio.bin", &vec![0x55u8; 2352 * 40])),
        rejected("not a PlayStation or PlayStation 2 disc")
    );
    // Broken cue text is unchanged (checked before any probe).
    let broken = write(
        d,
        "lost.cue",
        b"FILE \"gone.bin\" BINARY\n  TRACK 01 MODE2/2352\n",
    );
    assert_eq!(
        system_of(&broken),
        rejected("track file 'gone.bin' not found — fix the FILE line or rename the file")
    );
    // Every unclaimed `.bin` under the Atari size caps is DAT-pending, with the PS2
    // text as its reason; the 16 B floor of `ps2` does not steal them from the gate.
    let small: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
    write(d, "homebrew.bin", &small);
    let found = discover_rom_files(d, &ALL_SYSTEMS);
    // The broken cue stays a problem row; every `.bin` goes to the DAT gate.
    assert_eq!(found.failures.len(), 1, "{:?}", found.failures);
    assert!(found.failures[0].path.ends_with("lost.cue"));
    let mut pending: Vec<(&str, &str)> = found
        .dat_pending
        .iter()
        .map(|f| {
            let name = Path::new(&f.path).file_name().unwrap().to_str().unwrap();
            (name, f.reason.as_str())
        })
        .collect();
    pending.sort_unstable();
    let text = "not a PlayStation or PlayStation 2 disc";
    assert_eq!(
        pending,
        [
            ("audio.bin", text),
            ("homebrew.bin", text),
            ("neither.bin", text)
        ]
    );
    for file in &found.dat_pending {
        let candidates: Vec<&str> = file.candidates.iter().map(|s| s.id).collect();
        assert_eq!(candidates, ["atari2600", "atari7800"], "{}", file.path);
    }
}
