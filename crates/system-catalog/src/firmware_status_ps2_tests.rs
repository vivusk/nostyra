//! PS2 ROMDIR scan: valid dumps listed with labels, sidecars and garbage skipped,
//! folder and region rows from the rule; PCSX ReARMed / Flycast untouched.

use std::path::Path;

use super::*;
use crate::firmware_status::{bios_regions, BIOS_REGIONS};
use crate::ps2_bios_test_image::{ps2_bios_image, ps2_rom1_like_image, RETAIL_TABLE_OFFSET};
use crate::ps2_core_specs::PCEE2;

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::write(dir.join(name), bytes).unwrap_or_else(|e| panic!("{e}"));
}

fn bios_dir(system_dir: &Path) -> std::path::PathBuf {
    let dir = firmware_dir(system_dir, &PCEE2);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

#[test]
fn ps2_folder_and_region_rows_come_from_the_rule() {
    let system = Path::new("sys");
    assert_eq!(
        firmware_dir(system, &PCEE2),
        system.join("pcsx2").join("bios")
    );
    assert_eq!(bios_regions(&PCEE2), BIOS_REGIONS, "dumps are regional");
    assert!(PCEE2.firmware.is_empty(), "no md5 list");
}

#[test]
fn valid_dumps_are_listed_with_labels_in_name_order() {
    let temp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let dir = bios_dir(temp.path());
    write(
        &dir,
        "scph77001.bin",
        &ps2_bios_image("0220AC20060210", 0x2780),
    );
    write(
        &dir,
        "SCPH-39001_BIOS_V7_USA_160.BIN",
        &ps2_bios_image("0160AC20020207", RETAIL_TABLE_OFFSET),
    );
    let report = scan_firmware(temp.path(), &PCEE2);
    let names: Vec<&str> = report.files.iter().map(|f| f.file_name.as_str()).collect();
    assert_eq!(names, ["SCPH-39001_BIOS_V7_USA_160.BIN", "scph77001.bin"]);
    let first = &report.files[0];
    assert!(first.valid && first.known.is_none());
    assert_eq!(first.region, "US");
    assert_eq!(first.label.as_deref(), Some("v1.60 · USA · 2002-02-07"));
    assert_eq!(first.ps2_version.as_ref().map(|v| v.minor), Some(60));
    assert_eq!(
        report.files[1].label.as_deref(),
        Some("v2.20 · USA · 2006-02-10")
    );
    assert_eq!(report.valid().count(), 2);
}

#[test]
fn sidecars_and_garbage_are_never_listed() {
    let temp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    let dir = bios_dir(temp.path());
    write(
        &dir,
        "scph77001.bin",
        &ps2_bios_image("0220AC20060210", 0x2780),
    );
    write(&dir, "scph77001.rom1", &ps2_rom1_like_image()); // 4 MiB, RESET, no ROMVER
    write(&dir, "scph77001.nvm", &[0u8; 1024]);
    write(&dir, "scph77001.mec", &[0u8; 4]);
    write(&dir, "scph77001.erom", &vec![0u8; 2 * 1024 * 1024]);
    write(&dir, "zeros.bin", &vec![0u8; 4 * 1024 * 1024]);
    write(&dir, "notes.txt", b"not a bios");
    std::fs::create_dir_all(dir.join("4mib-folder")).unwrap_or_else(|e| panic!("{e}"));
    let report = scan_firmware(temp.path(), &PCEE2);
    let names: Vec<&str> = report.files.iter().map(|f| f.file_name.as_str()).collect();
    assert_eq!(names, ["scph77001.bin"]);
}

#[test]
fn missing_folder_and_other_cores_folders_are_empty() {
    let temp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
    assert!(scan_firmware(temp.path(), &PCEE2).files.is_empty());
    // A PS2 dump in the PlayStation folder is nobody's BIOS.
    write(
        temp.path(),
        "scph39001.bin",
        &ps2_bios_image("0160AC20020207", RETAIL_TABLE_OFFSET),
    );
    assert!(scan_firmware(temp.path(), &PCEE2).files.is_empty());
    assert!(scan_firmware(temp.path(), &crate::core_specs::PCSX_REARMED)
        .files
        .is_empty());
}
