//! Web DAT-pending logic on OS files (the default content source).

use rom_identity::hash_rom_bytes;
use system_catalog::{ATARI2600, ATARI7800};

use super::*;

fn dat_of(rom: &[u8]) -> DatIndex {
    let hash = hash_rom_bytes(rom, &ATARI7800);
    DatIndex::from_text(&format!(
        "game (\n\tname \"Fake (USA)\"\n\trom ( name \"Fake (USA).bin\" size {} crc {} sha1 {} )\n)\n",
        hash.size, hash.crc32, hash.sha1
    ))
    .unwrap()
}

#[test]
fn candidates_hit_and_hint_follow_the_desktop_rules() {
    let dir = tempfile::Builder::new()
        .prefix("nostyra-web-pending-")
        .tempdir()
        .unwrap();
    let rom: Vec<u8> = (0..16384u32).map(|i| (i % 251) as u8).collect();
    let bin = dir.path().join("game.bin");
    std::fs::write(&bin, &rom).unwrap();
    assert_eq!(candidates(&bin), ["atari2600", "atari7800"]);
    let a26 = dir.path().join("game.a26");
    std::fs::write(&a26, &rom).unwrap();
    assert!(candidates(&a26).is_empty(), "named extensions classify");
    let dat = dat_of(&rom);
    assert_eq!(dat_hit(&bin, ATARI7800.id, &dat), Ok(true));
    assert_eq!(dat_hit(&bin, ATARI2600.id, &DatIndex::empty()), Ok(false));
    assert!(dat_hit(&dir.path().join("missing.bin"), ATARI7800.id, &dat).is_err());
    let ids = candidates(&bin);
    assert!(hint("not a PlayStation or PlayStation 2 disc", &ids)
        .ends_with("(homebrew: use .a26 / .a78)"));
}
