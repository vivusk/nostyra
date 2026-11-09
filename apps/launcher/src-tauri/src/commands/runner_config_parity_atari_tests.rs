//! Parity cases for the Atari systems (M17): Stella (`.a26`), ProSystem (`.a78`) and
//! Handy (`.o`) with tiny in-code ROMs (`--print-config` stops after the load, so the
//! code never needs to do anything). Proves the registry core ids `stella`,
//! `prosystem` and `handy` are the cores' `core_layer_id`s.

use super::check_parity;

fn parity(system: &str, file: &str, rom: &[u8]) {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-atari-")
        .tempdir()
        .unwrap();
    let path = temp.path().join(file);
    std::fs::write(&path, rom).unwrap();
    check_parity(system, &path, |_| {});
}

#[test]
fn atari2600_launcher_resolution_matches_runner_print_config() {
    let mut rom = vec![0x4C, 0x00, 0xF0]; // JMP $F000
    rom.resize(4096, 0xEA);
    rom[0xFFC..0xFFE].copy_from_slice(&[0x00, 0xF0]); // reset vector
    parity("atari2600", "Parity.a26", &rom);
}

#[test]
fn atari7800_launcher_resolution_matches_runner_print_config() {
    let mut rom = vec![0u8; 128];
    rom[1..10].copy_from_slice(b"ATARI7800");
    rom[49..53].copy_from_slice(&16384u32.to_be_bytes());
    rom.extend_from_slice(&[0x4C, 0x00, 0xC0]); // JMP $C000
    rom.resize(128 + 16384, 0xEA);
    let end = rom.len();
    rom[end - 4..end - 2].copy_from_slice(&[0x00, 0xC0]); // reset vector
    parity("atari7800", "Parity.a78", &rom);
}

#[test]
fn lynx_launcher_resolution_matches_runner_print_config() {
    // BS93: BRA +8, load $0200, size 16, "BS93", then BRA -2 (loop) + NOPs (the
    // registry minimum is 16 bytes).
    let mut rom = vec![
        0x80, 0x08, 0x02, 0x00, 0x00, 0x10, b'B', b'S', b'9', b'3', 0x80, 0xFE,
    ];
    rom.resize(16, 0xEA);
    parity("lynx", "Parity.o", &rom);
}
