//! Vendored Redump PS2 data (DAT + developer metadat, placeholders for the four
//! attributes libretro-database lacks) found through the PS2 serial rule: the
//! developer's Berwick Saga disc by serial + size, variant-suffixed duplicate blocks
//! by the 10-char key.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use game_library::SystemCatalogs;
use system_catalog::PS2;

#[test]
fn vendored_ps2_data_knows_the_redump_serials() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    let ps2 = catalogs.get(PS2.id);
    assert_eq!(
        ps2.dat.len(),
        13_618,
        "Redump PS2 game blocks at DAT_COMMIT 9a899f5"
    );
    assert!(
        ps2.metadata.is_complete(),
        "placeholders for the 404 attributes"
    );
    // The developer's disc: `BOOT2 = cdrom0:\SLPS_254.97;1`, 4 321 804 288 bytes.
    let berwick = ps2
        .dat
        .lookup_serial("SLPS-25497", Some(4_321_804_288))
        .expect("DAT entry");
    assert!(
        berwick.name.starts_with(
            "TearRingSaga Series - Berwick Saga - Lazberia Chronicle Chapter 174 (Japan)"
        ),
        "{}",
        berwick.name
    );
    assert_eq!(berwick.size, 4_321_804_288);
    // Variant-suffixed duplicate blocks collapse to the disc's 10-char serial.
    let story = ps2
        .dat
        .lookup_serial("SLPM-65002", None)
        .expect("SLPM-65002");
    assert!(
        story.name.starts_with("0 Story (Japan) (Disc 1)"),
        "{}",
        story.name
    );
    let sotc = ps2
        .dat
        .lookup_serial("SCES-53326", None)
        .expect("SCES-53326");
    assert!(
        sotc.name.starts_with("Shadow of the Colossus (Europe"),
        "{}",
        sotc.name
    );
    // Redump's multi-dash serial (`SLES---54150`) meets the disc's `SLES-54150`.
    let bionicle = ps2
        .dat
        .lookup_serial("SLES-54150", None)
        .expect("SLES-54150");
    assert!(
        bionicle.name.starts_with("Bionicle Heroes (Europe)"),
        "{}",
        bionicle.name
    );
    // Developer metadat (the only PS2 attribute file upstream) is serial-keyed.
    let meta = ps2.metadata.lookup_serial("SLUS-20267").expect("metadata");
    assert_eq!(meta.developer.as_deref(), Some("CyberConnect2"));
    assert_eq!(meta.release_year, Some(2003));
    assert!(ps2.dat.lookup_serial("NSTR-00001", None).is_none());
}
