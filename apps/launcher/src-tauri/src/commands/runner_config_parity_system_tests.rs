//! Parity cases for the non-SNES systems (one game per core); the harness is in the
//! parent module (`check_parity`).

use super::{check_parity, repo};

/// Same check for an NES game on its own core (`config/core/mesen.toml`): proves the
/// launcher's registry core id equals `core_layer_id` of the core's `library_name`.
#[test]
fn nes_launcher_resolution_matches_runner_print_config() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-nes-rom-")
        .tempdir()
        .unwrap();
    let rom = temp.path().join("Parity.nes");
    std::fs::write(&rom, minimal_nrom()).unwrap();
    check_parity("nes", &rom);
}

/// GBA game on mGBA (`config/core/mgba.toml`, shared by gb/gbc/gba): proves
/// `core_layer_id("mGBA") == "mgba"`, the registry core id.
#[test]
fn gba_launcher_resolution_matches_runner_print_config() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-gba-rom-")
        .tempdir()
        .unwrap();
    let rom = temp.path().join("Parity.gba");
    std::fs::write(&rom, minimal_gba()).unwrap();
    check_parity("gba", &rom);
}

/// PS1 disc on PCSX ReARMed (`config/core/pcsx-rearmed.toml`): proves
/// `core_layer_id("PCSX-ReARMed") == "pcsx-rearmed"`, the registry core id, and that
/// the runner classifies a bare `.bin` as `psx` by content.
#[test]
fn psx_launcher_resolution_matches_runner_print_config() {
    use disc_image::test_images::{ps1_sectors, to_raw, US_LICENCE};
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-psx-disc-")
        .tempdir()
        .unwrap();
    let rom = temp.path().join("Parity.bin");
    let sectors = ps1_sectors(Some(US_LICENCE), r"BOOT = cdrom:\SLUS_012.34;1");
    std::fs::write(&rom, to_raw(&sectors, 2)).unwrap();
    check_parity("psx", &rom);
}

/// Mega Drive `.bin` on Genesis Plus GX (`config/core/genesis_plus_gx.toml`, shared
/// by the four Sega systems): proves `core_layer_id("Genesis Plus GX") ==
/// "genesis_plus_gx"` and that the runner classifies a `.bin` with `SEGA` at 0x100
/// as `genesis` by content.
#[test]
fn genesis_launcher_resolution_matches_runner_print_config() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-genesis-rom-")
        .tempdir()
        .unwrap();
    let rom = temp.path().join("Parity.bin");
    let mut image = vec![0; 128 * 1024];
    image[0x100..0x110].copy_from_slice(b"SEGA MEGA DRIVE ");
    std::fs::write(&rom, image).unwrap();
    check_parity("genesis", &rom);
}

/// Nintendo 64 test ROM on Mupen64Plus-Next (`config/core/mupen64plus-next.toml`):
/// proves `core_layer_id("Mupen64Plus-Next") == "mupen64plus-next"`. The core asks
/// for OpenGL inside `retro_load_game`, so `--print-config` skips (exit 6) without it.
#[test]
fn n64_launcher_resolution_matches_runner_print_config() {
    check_parity("n64", &repo("test-assets/roms/nostyra-test.z64"));
}

/// Dreamcast GDI on Flycast (`config/core/flycast.toml`): proves
/// `core_layer_id("Flycast") == "flycast"`, the registry core id. A synthetic GDI
/// (IP.BIN only) is enough: `--print-config` stops after `retro_load_game`, before the
/// HLE BIOS looks for a boot file. Skips (exit 6) without OpenGL, like N64.
#[test]
fn dreamcast_launcher_resolution_matches_runner_print_config() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-dc-disc-")
        .tempdir()
        .unwrap();
    let gdi = disc_image::test_gd_images::write_gdi(temp.path(), "parity", "NSTY-00001", false);
    check_parity("dreamcast", &gdi);
}

/// PSP UMD image on PPSSPP (`config/core/ppsspp.toml`): proves
/// `core_layer_id("PPSSPP") == "ppsspp"`. The in-code test UMD (PARAM.SFO, stub
/// EBOOT.BIN) is enough for `--print-config`; the runner first copies the core's
/// system files into the temp data dir. Skips (exit 6) without OpenGL, like N64.
#[test]
fn psp_launcher_resolution_matches_runner_print_config() {
    use disc_image::test_images::to_iso;
    use disc_image::test_psp_images::psp_iso_sectors;
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-psp-disc-")
        .tempdir()
        .unwrap();
    let iso = temp.path().join("parity.iso");
    std::fs::write(&iso, to_iso(&psp_iso_sectors())).unwrap();
    check_parity("psp", &iso);
}

/// Nintendo DS homebrew on melonDS DS (`config/core/melonds_ds.toml`): proves
/// `core_layer_id("melonDS DS") == "melonds_ds"`, the registry core id, and that the
/// runner classifies a `.nds` by its header CRC.
#[test]
fn nds_launcher_resolution_matches_runner_print_config() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-parity-nds-rom-")
        .tempdir()
        .unwrap();
    let rom = temp.path().join("Parity.nds");
    std::fs::write(&rom, minimal_nds()).unwrap();
    check_parity("nds", &rom);
}

/// 4 KiB homebrew DS image (ARM9 @0x200 → 0x02000000, ARM7 @0x400 → 0x037F8000,
/// both `b .`, valid header CRC, no logo); enough for `--print-config`.
fn minimal_nds() -> Vec<u8> {
    let mut rom = vec![0; 0x1000];
    rom[0x0C..0x10].copy_from_slice(b"ZNSP");
    let mut put = |offset: usize, value: u32| {
        rom[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    };
    for (field, value) in [
        (0x20, 0x200),
        (0x24, 0x0200_0000),
        (0x28, 0x0200_0000),
        (0x2C, 0x200),
        (0x30, 0x400),
        (0x34, 0x037F_8000),
        (0x38, 0x037F_8000),
        (0x3C, 0x200),
        (0x80, 0x1000),
        (0x84, 0x4000),
        (0x200, 0xEAFF_FFFE),
        (0x400, 0xEAFF_FFFE),
    ] {
        put(field, value);
    }
    let crc = system_catalog::crc16_modbus(&rom[..0x15E]);
    rom[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
    rom
}

/// 64 KiB GBA image whose entry point branches to itself (byte 3 = 0xEA), with the
/// fixed header byte mGBA checks (0xB2 = 0x96); enough for `--print-config`.
fn minimal_gba() -> Vec<u8> {
    let mut rom = vec![0; 64 * 1024];
    rom[..4].copy_from_slice(&[0xFE, 0xFF, 0xFF, 0xEA]); // b .
    rom[0xB2] = 0x96;
    rom
}

/// NROM-256 that loops forever (enough for `--print-config`, which only loads it).
fn minimal_nrom() -> Vec<u8> {
    let mut rom = vec![b'N', b'E', b'S', 0x1A, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut prg = vec![0xEA; 32 * 1024];
    prg[..3].copy_from_slice(&[0x4C, 0x00, 0x80]); // JMP $8000
    prg[0x7FFA..].copy_from_slice(&[0x00, 0x80, 0x00, 0x80, 0x00, 0x80]);
    rom.extend(prg);
    rom.extend(vec![0; 8 * 1024]);
    rom
}
