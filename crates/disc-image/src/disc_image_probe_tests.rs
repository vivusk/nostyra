//! DiscImageProbe tests on synthetic images of every accepted layout.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::test_images::{
    iso_sectors, pbp_image, ps1_sectors, to_chd, to_iso, to_raw, write, US_LICENCE,
};

const BOOT: &str = "BOOT = cdrom:\\SLUS_012.34;1";

fn ps1_probe(path: &Path) -> Result<(), String> {
    DiscImageProbe.check(DiscSignature::PlayStation, path)
}

#[test]
fn accepts_ps1_images_in_every_layout() {
    let dir = tempfile::tempdir().unwrap();
    let sectors = ps1_sectors(Some(US_LICENCE), BOOT);
    let raw = to_raw(&sectors, 2);
    assert_eq!(ps1_probe(&write(dir.path(), "a.bin", &raw)), Ok(()));
    assert_eq!(
        ps1_probe(&write(dir.path(), "b.iso", &to_iso(&sectors))),
        Ok(())
    );
    assert_eq!(
        ps1_probe(&write(dir.path(), "c.chd", &to_chd(&raw, "MODE2_RAW"))),
        Ok(())
    );
    let cue = write(
        dir.path(),
        "a.cue",
        b"FILE \"a.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
    );
    assert_eq!(ps1_probe(&cue), Ok(()));
    let m3u = write(dir.path(), "game.m3u", b"#EXTM3U\na.cue\nb.iso\n");
    assert_eq!(ps1_probe(&m3u), Ok(()));
}

#[test]
fn rejects_non_ps1_content_with_one_reason() {
    let dir = tempfile::tempdir().unwrap();
    let ps2 = to_raw(&ps1_sectors(None, "BOOT2 = cdrom0:\\SLUS_200.62;1"), 2);
    let not = Err(NOT_PLAYSTATION.to_owned());
    assert_eq!(ps1_probe(&write(dir.path(), "ps2.bin", &ps2)), not);
    assert_eq!(
        ps1_probe(&write(dir.path(), "audio.bin", &vec![0x55u8; 2352 * 40])),
        not
    );
    assert_eq!(
        ps1_probe(&write(dir.path(), "short.bin", b"tiny file here!!")),
        not
    );
    let pc = to_iso(&iso_sectors("WIN32", None, &[]));
    assert_eq!(ps1_probe(&write(dir.path(), "pc.iso", &pc)), not);
}

#[test]
fn broken_sheets_name_the_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let cue = write(
        dir.path(),
        "Brigandine.cue",
        b"FILE \"Brigandine_VIET_D13.bin\" BINARY\n  TRACK 01 MODE2/2352\n",
    );
    assert_eq!(
        ps1_probe(&cue),
        Err(
            "track file 'Brigandine_VIET_D13.bin' not found — fix the FILE line or rename the file"
                .to_owned()
        )
    );
    let m3u = write(dir.path(), "g.m3u", b"Disc 1.cue\n");
    assert_eq!(
        ps1_probe(&m3u),
        Err("disc 'Disc 1.cue' not found — fix the playlist line or rename the file".to_owned())
    );
    let audio_cue = write(
        dir.path(),
        "audio.cue",
        b"FILE \"x.bin\" BINARY\nTRACK 01 AUDIO\n",
    );
    write(dir.path(), "x.bin", &[0u8; 2352]);
    assert_eq!(ps1_probe(&audio_cue), Err(NOT_PLAYSTATION.to_owned()));
    let nested = write(dir.path(), "n.m3u", b"g.m3u\n");
    assert_eq!(
        ps1_probe(&nested),
        Err("playlist entry 'g.m3u' is itself a playlist".to_owned())
    );
}

#[test]
fn pbp_needs_ps1_category_and_disc_id() {
    let dir = tempfile::tempdir().unwrap();
    let ps1 = write(dir.path(), "EBOOT.PBP", &pbp_image("ME", "SLUS00594"));
    assert_eq!(ps1_probe(&ps1), Ok(()));
    let psp = write(dir.path(), "psp.pbp", &pbp_image("UG", "ULUS10041"));
    assert_eq!(ps1_probe(&psp), Err(NOT_PLAYSTATION.to_owned()));
}

fn dc_probe(path: &Path) -> Result<(), String> {
    DiscImageProbe.check(DiscSignature::Dreamcast, path)
}

#[test]
fn accepts_dreamcast_images_in_every_container() {
    use crate::test_cdi_image::{cdi_image, CDI_V2};
    use crate::test_gd_images::{gd_chd, write_gdi, write_redump_gd_cue};
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let gdi = write_gdi(d, "g", "NSTY-00001", false);
    let cue = write_redump_gd_cue(d, "Game", "NSTY-00001");
    let chd = write(d, "g.chd", &gd_chd("NSTY-00001"));
    let cdi = write(d, "g.cdi", &cdi_image(CDI_V2, "NSTY-00001"));
    let m3u = write(d, "g.m3u", b"g.gdi\ng.chd\n");
    for path in [&gdi, &cue, &chd, &cdi, &m3u] {
        assert_eq!(dc_probe(path), Ok(()), "{}", path.display());
    }
    // Neither system claims the other's discs.
    assert_eq!(ps1_probe(&cue), Err(NOT_PLAYSTATION.into()));
    assert_eq!(ps1_probe(&gdi), Err(NOT_PLAYSTATION.into()));
    let ps1 = write(d, "p.bin", &to_raw(&ps1_sectors(Some(US_LICENCE), BOOT), 2));
    let ps1_cue = write(
        d,
        "p.cue",
        b"FILE \"p.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
    );
    assert_eq!(dc_probe(&ps1_cue), Err(NOT_DREAMCAST.into()));
    assert_eq!(dc_probe(&ps1), Err(NOT_DREAMCAST.into()));
}

#[test]
fn broken_dreamcast_sheets_say_why() {
    use crate::test_cdi_image::{cdi_image, CDI_V3};
    use crate::test_gd_images::write_gdi;
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let gdi = write_gdi(d, "g", "NSTY-00001", false);
    std::fs::rename(d.join("g-track03.bin"), d.join("moved.bin")).unwrap();
    let missing = dc_probe(&gdi).unwrap_err();
    assert!(
        missing.contains("track file 'g-track03.bin' not found"),
        "{missing}"
    );
    let junk = write(d, "junk.gdi", b"hello\n");
    assert_eq!(dc_probe(&junk), Err("not a valid GDI sheet".into()));
    let no_hd = write(d, "cd.gdi", b"1\n1 0 4 2352 moved.bin 0\n");
    assert_eq!(
        dc_probe(&no_hd),
        Err("GDI has no high-density data track".into())
    );
    let mut cdi = cdi_image(CDI_V3, "X");
    let at = cdi.len() - 8;
    cdi[at..at + 4].copy_from_slice(&0x8000_0001u32.to_le_bytes());
    let old = write(d, "old.cdi", &cdi);
    assert_eq!(
        dc_probe(&old),
        Err("unsupported DiscJuggler version".into())
    );
}
