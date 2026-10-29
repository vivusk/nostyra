//! Settings → BIOS view tests: PlayStation regions, Dreamcast md5 rule.

use super::*;
use system_catalog::PCSX_REARMED;

#[test]
fn view_reports_hle_without_files_and_a_known_file_for_every_region() {
    let dir = tempfile::tempdir().unwrap();
    let empty = core_view(&PCSX_REARMED, dir.path());
    assert!(empty.files.is_empty());
    assert!(empty.regions.iter().all(|r| r.file_name.is_none()));
    assert_eq!(empty.system_names, ["PlayStation"]);
    let mut image = vec![0u8; system_catalog::BIOS_BYTES as usize];
    image[..5].copy_from_slice(&[0x13, 0x00, 0x08, 0x3C, 0x3F]);
    std::fs::write(dir.path().join("scph7001.bin"), &image).unwrap();
    let view = core_view(&PCSX_REARMED, dir.path());
    assert_eq!(view.files[0].model.as_deref(), Some("SCPH-7001"));
    assert!(view
        .regions
        .iter()
        .all(|r| r.file_name.as_deref() == Some("scph7001.bin")));
}

#[test]
fn dreamcast_lists_one_all_regions_row_and_needs_a_known_md5() {
    let dir = tempfile::tempdir().unwrap();
    let empty = core_view(&system_catalog::FLYCAST, dir.path());
    assert_eq!(empty.system_names, ["Sega Dreamcast"]);
    assert_eq!(empty.regions.len(), 1);
    assert_eq!(empty.regions[0].label, "All regions");
    assert!(empty.folder.ends_with("dc"), "{}", empty.folder);
    std::fs::create_dir_all(dir.path().join("dc")).unwrap();
    let not_a_bios = vec![0u8; 2 * 1024 * 1024];
    std::fs::write(dir.path().join("dc/dc_boot.bin"), not_a_bios).unwrap();
    let view = core_view(&system_catalog::FLYCAST, dir.path());
    assert_eq!(view.files.len(), 1);
    assert!(!view.files[0].valid, "right name and size, unknown md5");
    assert_eq!(
        view.regions[0].file_name, None,
        "HLE, as the runner decides"
    );
}
