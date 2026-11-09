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

#[test]
fn playstation_2_rows_name_the_newest_dump_of_the_region_not_the_first_by_name() {
    use system_catalog::ps2_bios_test_image::ps2_bios_image;
    use system_catalog::PCEE2;
    let dir = tempfile::tempdir().unwrap();
    let empty = core_view(&PCEE2, dir.path());
    assert_eq!(empty.system_names, ["PlayStation 2"]);
    assert!(!empty.hle_fallback, "no built-in BIOS");
    assert_eq!(empty.pick_extensions, ["bin", "rom0", "BIN", "ROM0"]);
    assert_eq!(empty.regions.len(), 3);
    assert!(empty.regions.iter().all(|r| r.file_name.is_none()));
    assert!(
        empty.folder.ends_with("bios") && empty.folder.contains("pcsx2"),
        "{}",
        empty.folder
    );
    let bios = dir.path().join("pcsx2/bios");
    std::fs::create_dir_all(&bios).unwrap();
    // Four USA dumps in the user's naming (name order ≠ version order).
    for (name, romver) in [
        ("scph-39001_bios_v7_usa_160.bin", "0160AC20020207"),
        ("scph-70012_bios_v12_usa_200.bin", "0200AC20040614"),
        ("scph-77001_bios_v15_usa_220.bin", "0220AC20060210"),
        ("scph-90001_bios_v18_usa_230.bin", "0230AC20080220"),
        ("scph-10000_bios_v1_jap_100.bin", "0100JC20000117"),
    ] {
        std::fs::write(bios.join(name), ps2_bios_image(romver, 0x2740)).unwrap();
    }
    std::fs::write(bios.join("scph-77001_bios_v15_usa_220.nvm"), [0u8; 1024]).unwrap();
    let view = core_view(&PCEE2, dir.path());
    assert_eq!(view.files.len(), 5, "{view:?}");
    assert!(view.files.iter().all(|f| f.valid && f.model.is_none()));
    let us = view.regions.iter().find(|r| r.region == "US").unwrap();
    assert_eq!(
        us.file_name.as_deref(),
        Some("scph-90001_bios_v18_usa_230.bin")
    );
    let jp = view.regions.iter().find(|r| r.region == "JP").unwrap();
    assert_eq!(
        jp.file_name.as_deref(),
        Some("scph-10000_bios_v1_jap_100.bin")
    );
    let eu = view.regions.iter().find(|r| r.region == "EU").unwrap();
    assert_eq!(
        eu.file_name.as_deref(),
        Some("scph-90001_bios_v18_usa_230.bin"),
        "no EU: newest"
    );
    let v100 = view
        .files
        .iter()
        .find(|f| f.file_name.starts_with("scph-10000"))
        .unwrap();
    assert_eq!(v100.label.as_deref(), Some("v1.00 · Japan · 2000-01-17"));
    assert!(v100
        .warning
        .as_deref()
        .unwrap_or("")
        .contains("memory-card"));
    assert_eq!(v100.region, "JP");
    let v230 = view
        .files
        .iter()
        .find(|f| f.file_name.starts_with("scph-90001"))
        .unwrap();
    assert_eq!(v230.warning, None);
    assert_eq!(v230.label.as_deref(), Some("v2.30 · USA · 2008-02-20"));
}

/// Dev-only: the developer's own dumps (read-only; copied into a temp data dir only)
/// import under their stems with sidecars and show the spike's labels.
/// `cargo test -p nostyra-launcher developer_ps2 -- --ignored`
#[test]
#[ignore = "needs the developer's own BIOS dumps"]
fn developer_ps2_dumps_import_with_sidecars_and_labels() {
    use crate::commands::bios_import::{import_name, max_bios_bytes};
    use crate::commands::bios_sidecar_import::copy_sidecars;
    use crate::commands::ps2_bios_import::stored_stem;
    use system_catalog::{firmware_dir, PCEE2};
    let root = std::path::Path::new(r"E:\PCSX2\bios");
    let picks = [
        root.join(r"SCPH-39001_BIOS_V7_USA_160_(NTSC)\SCPH-39001_BIOS_V7_USA_160.BIN"),
        root.join(r"SCPH-77001_BIOS_V15_USA_220_(NTSC)\SCPH-77001_BIOS_V15_USA_220.ROM0"),
    ];
    let data = tempfile::tempdir().unwrap();
    let dir = firmware_dir(data.path(), &PCEE2);
    std::fs::create_dir_all(&dir).unwrap();
    for pick in &picks {
        let picked = pick.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = read_capped(pick, max_bios_bytes(&PCEE2)).unwrap();
        let name = import_name(&picked, &bytes, &PCEE2).unwrap();
        std::fs::write(dir.join(&name), &bytes).unwrap();
        let copied = copy_sidecars(pick, &dir, &stored_stem(&picked)).unwrap();
        eprintln!("{picked} -> {name} + {copied:?}");
    }
    let view = core_view(&PCEE2, data.path());
    for f in &view.files {
        eprintln!(
            "{} | {} | {} | valid {}",
            f.file_name,
            f.label.as_deref().unwrap_or("-"),
            f.region,
            f.valid
        );
    }
    let labels: Vec<&str> = view
        .files
        .iter()
        .filter_map(|f| f.label.as_deref())
        .collect();
    assert_eq!(
        labels,
        ["v1.60 · USA · 2002-02-07", "v2.20 · USA · 2006-02-10"]
    );
    let us = view.regions.iter().find(|r| r.region == "US").unwrap();
    assert_eq!(
        us.file_name.as_deref(),
        Some("scph-77001_bios_v15_usa_220.bin")
    );
    assert!(dir.join("scph-77001_bios_v15_usa_220.rom1").is_file());
    assert!(dir.join("scph-77001_bios_v15_usa_220.nvm").is_file());
    assert!(!dir.join("scph-77001_bios_v15_usa_220.diff").exists());
}
