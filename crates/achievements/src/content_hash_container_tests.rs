//! Containers the fixtures do not ship (CHD CD / GD / DVD, CDI, PS1 PBP), built in a
//! temp dir with `disc-image`'s `test-images` builders: our reader vs the default
//! reader's hash of the same content in a format it can read.

use disc_image::test_cdi_image::{cdi_image_with, CDI_V35};
use disc_image::test_gd_images::chd_v5;
use disc_image::test_images::{pbp_image, to_chd, to_raw, write};
use disc_image::test_psp_containers::dvd_chd;

use super::{default_hash, roms, DC_HASH, PSP_HASH, PSX_HASH};
use crate::content_hash;

#[test]
fn ps1_cd_chd_matches_its_cue_twin() {
    let dir = tempfile::tempdir().unwrap();
    let bin = std::fs::read(roms().join("nostyra-test-psx.bin")).unwrap();
    let chd = write(dir.path(), "psx.chd", &to_chd(&bin, "MODE2_RAW"));
    assert_eq!(default_hash(12, &chd), None);
    assert_eq!(content_hash(12, &chd, None).as_deref(), Some(PSX_HASH));
}

#[test]
fn psp_dvd_chd_matches_its_iso_twin() {
    let dir = tempfile::tempdir().unwrap();
    let iso = std::fs::read(roms().join("nostyra-test-psp.iso")).unwrap();
    let chd = write(dir.path(), "psp.chd", &dvd_chd(&iso, true));
    assert_eq!(content_hash(41, &chd, None).as_deref(), Some(PSP_HASH));
}

/// Track `n` of the DC fixture as 2048-byte sectors (tracks 1 / 3 are MODE1/2048).
fn dc_track(n: u32) -> Vec<Vec<u8>> {
    let name = format!("nostyra-test-dc (Track {n}).bin");
    let bytes = std::fs::read(roms().join(name)).unwrap();
    bytes.chunks(2048).map(<[u8]>::to_vec).collect()
}

#[test]
fn dreamcast_gd_chd_matches_its_gdi_twin() {
    let audio = std::fs::read(roms().join("nostyra-test-dc (Track 2).raw")).unwrap();
    let tracks = [
        ("MODE1_RAW", to_raw(&dc_track(1), 1)),
        ("AUDIO", audio),
        ("MODE1_RAW", to_raw(&dc_track(3), 1)),
    ];
    let (mut raw, mut metas) = (Vec::new(), Vec::new());
    for (n, (kind, data)) in tracks.iter().enumerate() {
        let frames = data.len() / 2352;
        raw.extend_from_slice(data);
        raw.resize(raw.len() + (frames.next_multiple_of(4) - frames) * 2352, 0);
        let text = format!(
            "TRACK:{} TYPE:{kind} SUBTYPE:NONE FRAMES:{frames} PAD:0 PREGAP:0 PGTYPE:MODE1 PGSUB:RW POSTGAP:0",
            n + 1
        );
        metas.push((*b"CHGD", text));
    }
    let dir = tempfile::tempdir().unwrap();
    let chd = write(dir.path(), "dc.chd", &chd_v5(&raw, &metas));
    assert_eq!(default_hash(40, &chd), None);
    assert_eq!(content_hash(40, &chd, None).as_deref(), Some(DC_HASH));
}

/// The data track sits at LBA 45000 so the fixture's absolute ISO9660 LBAs line up:
/// proves the first-data fallback and absolute reads, not a real MIL-CD layout (data
/// session near LBA 11700).
#[test]
fn dreamcast_cdi_matches_its_gdi_twin() {
    let dir = tempfile::tempdir().unwrap();
    let image = cdi_image_with(CDI_V35, &dc_track(3), 45_000 - 2);
    let cdi = write(dir.path(), "dc.cdi", &image);
    assert_eq!(default_hash(40, &cdi), None);
    assert_eq!(content_hash(40, &cdi, None).as_deref(), Some(DC_HASH));
}

#[test]
fn ps1_eboot_has_no_hash_but_a_psp_one_hashes_whole() {
    let dir = tempfile::tempdir().unwrap();
    let pbp = write(dir.path(), "EBOOT.PBP", &pbp_image("ME", "SLUS00594"));
    assert_eq!(
        content_hash(12, &pbp, None),
        None,
        "rcheevos v12.5.0: no PS1 PBP"
    );
    let psp = content_hash(41, &pbp, None);
    assert!(psp.is_some());
    assert_eq!(psp, default_hash(41, &pbp));
}
