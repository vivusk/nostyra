//! Folder walk over every registry system: files get their system from content,
//! never from folder names.
#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use std::collections::BTreeMap;

use game_library::discover_rom_files;
use system_catalog::ALL_SYSTEMS;

fn ines() -> Vec<u8> {
    let mut rom = vec![b'N', b'E', b'S', 0x1A, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    rom.resize(16 + 16 * 1024 + 8 * 1024, 0xEA);
    rom
}

#[test]
fn mixed_folder_is_classified_by_content() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-gl-discover-")
        .tempdir()
        .unwrap();
    let write = |rel: &str, bytes: &[u8]| {
        let path = temp.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    };
    let snes = vec![0u8; 64 * 1024];
    write("SNES/a.sfc", &snes);
    write("NES/b.nes", &ines());
    write("Favorites/c.NES", &ines());
    write("NES/d.sfc", &snes); // an SNES game in a folder named NES
    write("junk.txt", b"not a rom");
    write("bad.nes", &ines()[16..]); // headerless: probe fails
    write("tiny.nes", &ines()[..4096]); // below the NES minimum

    let found = discover_rom_files(temp.path(), &ALL_SYSTEMS);
    let by_name: BTreeMap<String, &str> = found
        .files
        .iter()
        .map(|file| {
            let name = std::path::Path::new(&file.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            (name, file.system.id)
        })
        .collect();
    let expected: BTreeMap<String, &str> = [
        ("a.sfc", "snes"),
        ("b.nes", "nes"),
        ("c.NES", "nes"),
        ("d.sfc", "snes"),
    ]
    .into_iter()
    .map(|(name, id)| (name.to_string(), id))
    .collect();
    assert_eq!(by_name, expected);
    assert_eq!(found.skipped, 1, "tiny.nes is out of size");
    assert_eq!(found.failures.len(), 1);
    assert!(found.failures[0].path.ends_with("bad.nes"));
    assert_eq!(found.failures[0].reason, "missing iNES header");
}

#[test]
fn walk_restricted_to_some_systems_ignores_the_others() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-gl-discover-")
        .tempdir()
        .unwrap();
    std::fs::write(temp.path().join("b.nes"), ines()).unwrap();
    std::fs::write(temp.path().join("bad.nes"), &ines()[16..]).unwrap();
    let found = discover_rom_files(temp.path(), &[&system_catalog::SNES]);
    assert!(found.files.is_empty());
    assert_eq!((found.skipped, found.failures.len()), (0, 0));
}

#[test]
fn bundled_test_roms_are_classified_by_content() {
    let roms = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-assets/roms");
    let found = discover_rom_files(&roms, &ALL_SYSTEMS);
    let system_of = |name: &str| {
        found
            .files
            .iter()
            .find(|f| f.path.ends_with(name))
            .map(|f| f.system.id)
    };
    assert_eq!(system_of("nostyra-test.nes"), Some("nes"));
    assert_eq!(system_of("nostyra-test.sfc"), Some("snes"));
    assert_eq!(system_of("nostyra-test.gb"), Some("gb"));
    assert_eq!(system_of("nostyra-test.gbc"), Some("gbc"));
    assert_eq!(system_of("nostyra-test.gba"), Some("gba"));
}

#[test]
fn bare_playstation_bins_are_listed_by_content() {
    use disc_image::test_images::{ps1_sectors, to_iso, to_raw, US_LICENCE};
    let temp = tempfile::Builder::new()
        .prefix("nostyra-gl-discover-psx-")
        .tempdir()
        .unwrap();
    let ps1 = ps1_sectors(Some(US_LICENCE), r"BOOT = cdrom:\SLUS_012.34;1");
    std::fs::write(temp.path().join("Game (USA).bin"), to_raw(&ps1, 2)).unwrap();
    std::fs::write(temp.path().join("Other.iso"), to_iso(&ps1)).unwrap();
    // PS2-like: same system id, no licence text.
    let ps2 = ps1_sectors(None, r"BOOT2 = cdrom0:\SLUS_200.62;1");
    std::fs::write(temp.path().join("ps2.bin"), to_raw(&ps2, 2)).unwrap();

    let found = discover_rom_files(temp.path(), &ALL_SYSTEMS);
    let mut psx: Vec<String> = found
        .files
        .iter()
        .map(|f| {
            assert_eq!(f.system.id, "psx");
            std::path::Path::new(&f.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    psx.sort();
    assert_eq!(psx, ["Game (USA).bin", "Other.iso"]);
    assert_eq!(found.failures.len(), 1);
    assert_eq!(found.failures[0].reason, "not a PlayStation disc");
}
