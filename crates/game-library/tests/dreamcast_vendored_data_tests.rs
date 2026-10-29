//! Vendored Dreamcast data (Redump DAT + serial-keyed developer metadat): IP.BIN
//! product numbers find their DAT entry and metadata through the serial rule.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use game_library::SystemCatalogs;
use system_catalog::DREAMCAST;

#[test]
fn vendored_dreamcast_data_matches_ip_bin_products() {
    let vendored =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third-party/libretro-database");
    let catalogs = SystemCatalogs::new(Some(vendored));
    let dc = catalogs.get(DREAMCAST.id);
    assert!(dc.dat.len() > 1_800);
    assert!(
        dc.metadata.is_complete(),
        "placeholders keep the index complete"
    );
    // A US disc's IP.BIN says MK-51064; Redump lists the US entry as "51064".
    let us = dc.dat.lookup_serial("MK-51064", None).expect("US entry");
    assert_eq!(us.name, "18 Wheeler - American Pro Trucker (USA)");
    let eu = dc.dat.lookup_serial("MK-51064-50", None).expect("EU entry");
    assert!(eu.name.contains("(Europe)"), "{}", eu.name);
    let jp = dc.dat.lookup_serial("HDR-0080", None).expect("JP entry");
    assert!(jp.name.contains("(Japan)"), "{}", jp.name);
    let developer = dc.metadata.lookup_serial("MK-51064").expect("developer");
    assert_eq!(developer.developer.as_deref(), Some("Sega AM2"));
}
