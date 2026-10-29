//! IP.BIN tests: parsing, and reading it through every container (same product).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::test_cdi_image::{cdi_image, CDI_V35};
use crate::test_gd_images::{
    gd_chd, hd_track_sectors, ip_bin_sector, write_gdi, write_redump_gd_cue,
};
use crate::test_images::{iso_sectors, to_iso, to_raw, write};

#[test]
fn parses_ip_bin_fields() {
    let sector: SectorData = ip_bin_sector("MK-51000", "SONIC ADVENTURE")
        .try_into()
        .unwrap();
    let ip = IpBin::parse(&sector).unwrap();
    assert_eq!(ip.product, "MK-51000");
    assert_eq!(ip.version, "V1.000");
    assert_eq!(ip.disc, "1/1");
    assert_eq!(ip.area, "JUE");
    assert_eq!(ip.title, "SONIC ADVENTURE");
    assert!(ip.has_area('U') && !ip.has_area('X'));
    let mut other = sector;
    other[..4].copy_from_slice(b"SEGA");
    other[4] = b'X';
    assert!(IpBin::parse(&other).is_none());
}

#[test]
fn every_container_yields_the_same_product() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let product = |path: &Path| read_ip_bin(path).unwrap().unwrap().product;
    assert_eq!(
        product(&write_gdi(d, "g", "NSTY-00001", false)),
        "NSTY-00001"
    );
    assert_eq!(
        product(&write_gdi(d, "q", "NSTY-00001", true)),
        "NSTY-00001"
    );
    assert_eq!(
        product(&write_redump_gd_cue(d, "Game", "NSTY-00001")),
        "NSTY-00001"
    );
    assert_eq!(
        product(&write(d, "g.chd", &gd_chd("NSTY-00001"))),
        "NSTY-00001"
    );
    assert_eq!(
        product(&write(d, "g.cdi", &cdi_image(CDI_V35, "NSTY-00001"))),
        "NSTY-00001"
    );
    let iso = write(d, "t3.iso", &to_iso(&hd_track_sectors("NSTY-00001", 2)));
    assert_eq!(product(&iso), "NSTY-00001");
    let raw = write(d, "t3.bin", &to_raw(&hd_track_sectors("NSTY-00001", 2), 1));
    assert_eq!(product(&raw), "NSTY-00001");
}

#[test]
fn non_dreamcast_discs_have_no_ip_bin() {
    let dir = tempfile::tempdir().unwrap();
    let iso = write(
        dir.path(),
        "ps.iso",
        &to_iso(&iso_sectors("PLAYSTATION", None, &[])),
    );
    assert_eq!(read_ip_bin(&iso).unwrap(), None);
    let m3u = write(dir.path(), "a.m3u", b"ps.iso\n");
    assert!(
        read_ip_bin(&m3u).is_err(),
        "playlists are resolved by callers"
    );
}
