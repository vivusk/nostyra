//! BIOS status: known names in any case, any-name fallback, invalid files, region
//! choice with fallback to another region's file, HLE without files.

use std::path::Path;

use super::*;
use crate::core_specs::PCSX_REARMED;

/// 512 KiB image with the `lui t0` signature; `region_mark` at 0x7FF51 (" E" / " J").
fn bios_image(region_mark: &[u8; 2]) -> Vec<u8> {
    let mut image = vec![0u8; BIOS_BYTES as usize];
    image[..5].copy_from_slice(&[0x13, 0x00, 0x08, 0x3C, 0x3F]);
    image[0x7FF51..0x7FF53].copy_from_slice(region_mark);
    image
}

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::write(dir.join(name), bytes).unwrap_or_else(|e| panic!("{e}"));
}

fn real(name: &str) -> BiosChoice {
    BiosChoice::Real {
        file_name: name.into(),
    }
}

#[test]
fn no_files_means_hle_everywhere() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let report = scan_firmware(dir.path(), &PCSX_REARMED);
    assert!(report.files.is_empty());
    for region in BIOS_REGIONS {
        assert_eq!(report.choice_for(region), BiosChoice::Hle);
    }
    let missing = scan_firmware(&dir.path().join("nope"), &PCSX_REARMED);
    assert!(missing.files.is_empty());
}

#[test]
fn a_single_us_bios_serves_every_region() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    write(dir.path(), "SCPH1001.BIN", &bios_image(b"  "));
    let report = scan_firmware(dir.path(), &PCSX_REARMED);
    assert_eq!(report.files.len(), 1);
    let file = &report.files[0];
    assert!(file.valid);
    assert_eq!(
        file.known.map(|k| k.model),
        Some("SCPH-1001"),
        "upper-case name known"
    );
    assert_eq!(file.region, "US");
    for region in BIOS_REGIONS {
        assert_eq!(report.choice_for(region), real("SCPH1001.BIN"), "{region}");
    }
}

#[test]
fn regional_files_win_and_unknown_names_are_guessed_by_content() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    write(dir.path(), "scph1001.bin", &bios_image(b"  "));
    write(dir.path(), "my-japanese-bios.rom", &bios_image(b" J"));
    write(dir.path(), "broken.bin", &vec![0u8; BIOS_BYTES as usize]);
    write(dir.path(), "notes.txt", b"not a bios");
    let report = scan_firmware(dir.path(), &PCSX_REARMED);
    let names: Vec<&str> = report.files.iter().map(|f| f.file_name.as_str()).collect();
    assert_eq!(
        names,
        ["scph1001.bin", "broken.bin", "my-japanese-bios.rom"]
    );
    assert!(!report.files[1].valid, "zero image fails the signature");
    assert_eq!(report.files[2].region, "JP");
    assert_eq!(report.choice_for("JP"), real("my-japanese-bios.rom"));
    assert_eq!(report.choice_for("US"), real("scph1001.bin"));
    assert_eq!(
        report.choice_for("EU"),
        real("scph1001.bin"),
        "first valid file"
    );
}

#[test]
fn signature_accepts_both_core_marks() {
    assert!(is_valid_bios(&bios_image(b"  ")[..BIOS_HEAD_BYTES]));
    let mut ps = vec![0u8; BIOS_HEAD_BYTES];
    ps[0x12C..0x12C + 13].copy_from_slice(b"PS compatible");
    assert!(is_valid_bios(&ps));
    assert!(!is_valid_bios(&[0u8; 16]));
}

#[test]
fn dreamcast_reads_known_names_in_its_subfolder_at_their_size() {
    use crate::dreamcast_core_spec::FLYCAST;
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(firmware_dir(dir.path(), &FLYCAST), dir.path().join("dc"));
    assert_eq!(bios_regions(&FLYCAST), [ANY_REGION]);
    assert_eq!(bios_regions(&PCSX_REARMED), BIOS_REGIONS);
    let dc = dir.path().join("dc");
    std::fs::create_dir_all(&dc).unwrap_or_else(|e| panic!("{e}"));
    write(dir.path(), "dc_boot.bin", &vec![0u8; 2 * 1024 * 1024]); // wrong folder
    write(&dc, "other.bin", &vec![0u8; 2 * 1024 * 1024]); // unknown name
    write(&dc, "DC_BOOT.BIN", &[0u8; 1024]); // wrong size
    assert_eq!(
        scan_firmware(dir.path(), &FLYCAST).choice_for(ANY_REGION),
        BiosChoice::Hle
    );
    write(&dc, "DC_BOOT.BIN", &vec![0u8; 2 * 1024 * 1024]);
    let report = scan_firmware(dir.path(), &FLYCAST);
    assert_eq!(report.files.len(), 1, "{report:?}");
    assert_eq!(report.files[0].region, ANY_REGION);
    assert_eq!(report.choice_for(ANY_REGION), real("DC_BOOT.BIN"));
    // PlayStation never looks into dc/.
    assert!(scan_firmware(dir.path(), &PCSX_REARMED).files.is_empty());
}
