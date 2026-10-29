//! Vendored PSP data (Redump DAT + serial-keyed metadat) found through the PSP serial
//! rule, and PSP images the scan cannot use, reported with a reason.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod common;

use std::path::Path;

use common::TempLibrary;
use disc_image::test_images::to_iso;
use disc_image::test_psp_containers::{cso, pbp};
use disc_image::test_psp_images::{game_sfo, iso_tree, psp_iso_sectors};
use game_library::{DatIndex, SystemCatalogs};
use system_catalog::PSP;

#[test]
fn vendored_psp_data_matches_disc_serials() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    let psp = catalogs.get(PSP.id);
    assert!(psp.dat.len() > 4_000);
    assert!(psp.metadata.is_complete(), "maxusers placeholder");
    // PARAM.SFO says ULUS10041; Redump lists ULUS-10041.
    let gta = psp.dat.lookup_serial("ULUS10041", None).expect("DAT entry");
    assert!(
        gta.name
            .starts_with("Grand Theft Auto - Liberty City Stories (USA)"),
        "{}",
        gta.name
    );
    // Redump's `ULES-01513/E` form is found by the plain serial.
    assert!(psp.dat.lookup_serial("ULES-01513", None).is_some());
    // Upstream PSP developer metadat is mostly names only; genre and year are there.
    let meta = psp.metadata.lookup_serial("ULUS10041").expect("metadata");
    assert_eq!(meta.genre.as_deref(), Some("Adventure"));
    assert_eq!(meta.release_year, Some(2005));
}

#[test]
fn unreadable_psp_images_are_reported_with_a_reason() {
    let lib = TempLibrary::new("psp-bad");
    let iso = to_iso(&psp_iso_sectors());
    let mut zso = cso(&iso, 2048, 0);
    zso[..4].copy_from_slice(b"ZISO");
    lib.write("Packed.cso", &zso);
    lib.write(
        "Movie.iso",
        &to_iso(&iso_tree("UMD VIDEO", &[("UMD_VIDEO/X.BIN", b"x")])),
    );
    lib.write("Odd/EBOOT.PBP", &pbp(&game_sfo("EG"), b"PSISOIMG0000"));
    let summary = lib.scan(&DatIndex::default());
    assert_eq!(summary.added, 0);
    let mut reasons: Vec<(String, String)> = summary
        .failures
        .iter()
        .map(|f| {
            (
                f.path.file_name().unwrap().to_str().unwrap().to_owned(),
                f.reason.clone(),
            )
        })
        .collect();
    reasons.sort();
    assert_eq!(reasons.len(), 3, "{reasons:?}");
    assert!(reasons[0].1.contains("not a PSP eboot"), "{reasons:?}");
    // The GameCube / Wii probes come later on `.iso`; their plain "not a … disc"
    // never replaces PSP's specific reason.
    assert!(reasons[1].1.contains("UMD Video/Audio disc"), "{reasons:?}");
    assert!(reasons[2].1.contains("ZSO is not supported"), "{reasons:?}");
}
