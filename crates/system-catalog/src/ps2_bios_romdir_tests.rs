//! ROMDIR validator: version / region / date decode, labels, invalid shapes.

use super::*;
use crate::ps2_bios_test_image::{
    ps2_bios_image, ps2_bios_image_sized, ps2_rom1_like_image, RETAIL_TABLE_OFFSET,
};

fn parse(romver: &str) -> Ps2BiosVersion {
    parse_ps2_bios(&ps2_bios_image(romver, RETAIL_TABLE_OFFSET)).expect("valid image")
}

#[test]
fn decodes_the_users_scph_39001_romver() {
    let v = parse("0160AC20020207");
    assert_eq!((v.major, v.minor), (1, 60));
    assert_eq!(v.region, 'A');
    assert!(v.console);
    assert_eq!(v.date, "2002-02-07");
    assert_eq!(v.label(), "v1.60 · USA · 2002-02-07");
    assert_eq!(v.region_code(), "US");
    assert!(!v.is_discouraged());
}

#[test]
fn europe_japan_and_devel_units() {
    let eu = parse("0200EC20040614");
    assert_eq!(eu.label(), "v2.00 · Europe · 2004-06-14");
    assert_eq!(eu.region_code(), "EU");
    let jp = parse("0100JC20000117");
    assert_eq!(jp.label(), "v1.00 · Japan · 2000-01-17");
    assert_eq!(jp.region_code(), "JP");
    assert!(jp.is_discouraged(), "SCPH-10000 memory-card bugs");
    let devel = parse("0170TD20030227");
    assert_eq!(devel.label(), "v1.70 · T10K · 2003-02-27 · devel");
    assert_eq!(devel.region_code(), ANY_REGION, "no Settings region row");
    assert!(!devel.console);
    let unknown = parse("0230ZC20080220");
    assert_eq!(unknown.label(), "v2.30 · region ? · 2008-02-20");
}

#[test]
fn versions_order_by_number_then_date() {
    let v160 = parse("0160AC20020207");
    let v220 = parse("0220AC20060210");
    let v230 = parse("0230AC20080220");
    let v200_eu = parse("0200EC20040614");
    assert!(v160 < v200_eu && v200_eu < v220 && v220 < v230);
    let later_build = parse("0160AC20021231");
    assert!(v160 < later_build, "same version, newer date");
}

#[test]
fn table_anywhere_in_the_image_is_found() {
    let late = ps2_bios_image("0160AC20020207", 1 << 20);
    assert_eq!(parse_ps2_bios(&late).map(|v| v.minor), Some(60));
    let at_start = ps2_bios_image("0160AC20020207", 0);
    assert_eq!(parse_ps2_bios(&at_start).map(|v| v.minor), Some(60));
    let eight = ps2_bios_image_sized("0160AC20020207", 0x2780, 8 * 1024 * 1024);
    assert!(
        parse_ps2_bios(&eight).is_some(),
        "8 MiB upper bound inclusive"
    );
}

#[test]
fn rom1_diff_and_garbage_are_not_a_bios() {
    assert_eq!(
        parse_ps2_bios(&ps2_rom1_like_image()),
        None,
        "RESET, no ROMVER"
    );
    assert_eq!(
        parse_ps2_bios(&vec![0u8; 4 * 1024 * 1024]),
        None,
        "no RESET"
    );
    let mut noise = ps2_bios_image("0160AC20020207", RETAIL_TABLE_OFFSET);
    noise[RETAIL_TABLE_OFFSET..RETAIL_TABLE_OFFSET + 16].fill(0xFF);
    assert_eq!(parse_ps2_bios(&noise), None, "RESET overwritten");
}

#[test]
fn sizes_outside_four_to_eight_mib_are_refused() {
    let three = ps2_bios_image_sized("0160AC20020207", 0x2740, 3 * 1024 * 1024);
    assert_eq!(parse_ps2_bios(&three), None);
    let nine = ps2_bios_image_sized("0160AC20020207", 0x2740, 9 * 1024 * 1024);
    assert_eq!(parse_ps2_bios(&nine), None);
    let short = ps2_bios_image_sized("0160AC20020207", 0x2740, 4 * 1024 * 1024 - 1);
    assert_eq!(parse_ps2_bios(&short), None);
}

#[test]
fn truncated_or_crafted_tables_never_panic() {
    let mut image = ps2_bios_image("0160AC20020207", RETAIL_TABLE_OFFSET);
    // ROMVER record pushed past the image end by a huge EXTINFO size.
    let extinfo = RETAIL_TABLE_OFFSET + 2 * 16;
    image[extinfo + 12..extinfo + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(parse_ps2_bios(&image), None);
    // Table at the very end of the image: records run out.
    let tail = 4 * 1024 * 1024 - 16;
    let mut image = vec![0u8; 4 * 1024 * 1024];
    image[tail..tail + 5].copy_from_slice(b"RESET");
    assert_eq!(parse_ps2_bios(&image), None);
    // Non-digit version text.
    assert_eq!(
        parse_ps2_bios(&ps2_bios_image("01x0AC20020207", 0x2740)),
        None
    );
    assert_eq!(
        parse_ps2_bios(&ps2_bios_image("0160AC2002020x", 0x2740)),
        None
    );
    // Multi-byte UTF-8 straddling the version/flags split point.
    assert_eq!(
        parse_ps2_bios(&ps2_bios_image("016\u{e9}AC2002020", 0x2740)),
        None
    );
}

/// Dev-only: the developer's own SCPH-39001 / SCPH-77001 dumps (read-only) decode
/// to the versions the spike recorded. `cargo test -p system-catalog developer_ -- --ignored`
#[test]
#[ignore = "needs the developer's own BIOS dumps"]
fn developer_dumps_decode_like_the_spike_recorded() {
    let root = std::path::Path::new(r"E:\PCSX2\bios");
    let bin = root.join(r"SCPH-39001_BIOS_V7_USA_160_(NTSC)\SCPH-39001_BIOS_V7_USA_160.BIN");
    let v = parse_ps2_bios(&std::fs::read(&bin).unwrap_or_default()).expect("39001 valid");
    assert_eq!(v.label(), "v1.60 · USA · 2002-02-07");
    let dir = root.join("SCPH-77001_BIOS_V15_USA_220_(NTSC)");
    let rom0 = std::fs::read(dir.join("SCPH-77001_BIOS_V15_USA_220.ROM0")).unwrap_or_default();
    let v = parse_ps2_bios(&rom0).expect("77001 ROM0 valid");
    assert_eq!(v.label(), "v2.20 · USA · 2006-02-10");
    let rom1 = std::fs::read(dir.join("SCPH-77001_BIOS_V15_USA_220.ROM1")).unwrap_or_default();
    assert_eq!(parse_ps2_bios(&rom1), None, "ROM1: RESET without ROMVER");
    let diff = std::fs::read(dir.join("SCPH-77001_BIOS_V15_USA_220.DIFF")).unwrap_or_default();
    assert_eq!(parse_ps2_bios(&diff), None, "DIFF: no RESET");
}
