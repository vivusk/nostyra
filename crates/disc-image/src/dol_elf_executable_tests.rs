//! `.dol` / `.elf` checks and the GC/Wii HID4 split on synthetic executables.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::io::Cursor;

use super::*;
use crate::test_nintendo_images::{dol, elf, BLR, CODE_ADDR, MTSPR_HID4_R0};

const GC: Option<NintendoDiscKind> = Some(NintendoDiscKind::GameCube);
const WII: Option<NintendoDiscKind> = Some(NintendoDiscKind::Wii);
/// `mtspr HID4, r1`: another source register still matches under the mask.
const MTSPR_HID4_R1: u32 = MTSPR_HID4_R0 | 1 << 21;
const EM_386: u16 = 3;

fn kind(image: &[u8], ext: &str) -> Option<NintendoDiscKind> {
    executable_kind_from(&mut Cursor::new(image), ext, image.len() as u64).unwrap()
}

fn put(image: &mut [u8], at: usize, value: u32) {
    image[at..at + 4].copy_from_slice(&value.to_be_bytes());
}

#[test]
fn dol_without_hid4_is_gamecube_with_hid4_is_wii() {
    assert_eq!(kind(&dol(&[BLR; 8], &[]), "dol"), GC);
    assert_eq!(kind(&dol(&[BLR, MTSPR_HID4_R0, BLR], &[]), "dol"), WII);
    assert_eq!(kind(&dol(&[BLR, MTSPR_HID4_R1], &[]), "DOL"), WII);
}

#[test]
fn hid4_in_a_data_section_only_stays_gamecube() {
    assert_eq!(kind(&dol(&[BLR; 4], &[MTSPR_HID4_R0; 4]), "dol"), GC);
}

#[test]
fn dol_section_in_wii_mem2_is_wii() {
    let mut image = dol(&[BLR; 4], &[BLR; 4]);
    put(&mut image, 0x64, 0x9000_0000); // data 0 load address
    assert_eq!(kind(&image, "dol"), WII);
}

#[test]
fn broken_dol_section_tables_are_refused() {
    let good = dol(&[BLR; 8], &[BLR; 2]);
    let broken = |at: usize, value: u32| {
        let mut image = good.clone();
        put(&mut image, at, value);
        kind(&image, "dol")
    };
    assert_eq!(broken(0x00, 0x1000), None, "text past the end of the file");
    assert_eq!(broken(0x48, 0x0000_3100), None, "text outside RAM");
    assert_eq!(
        broken(0x64, 0x817F_FFFC),
        None,
        "data crosses the end of MEM1"
    );
    assert_eq!(broken(0xE0, CODE_ADDR + 0x100), None, "entry outside text");
    assert_eq!(broken(0x90, 0), None, "no text section");
    assert_eq!(kind(&good[..0xFF], "dol"), None, "short header");
}

#[test]
fn powerpc_elf_is_split_by_hid4() {
    assert_eq!(kind(&elf(20, &[BLR; 4]), "elf"), GC);
    assert_eq!(kind(&elf(20, &[BLR, MTSPR_HID4_R0]), "ELF"), WII);
}

#[test]
fn non_powerpc_or_broken_elf_is_refused() {
    assert_eq!(kind(&elf(EM_386, &[BLR; 4]), "elf"), None, "x86");
    let mut little = elf(20, &[BLR; 4]);
    little[5] = 1;
    assert_eq!(kind(&little, "elf"), None, "little-endian");
    let mut past_end = elf(20, &[BLR; 4]);
    put(&mut past_end, 52 + 16, 0x1000); // p_filesz
    assert_eq!(kind(&past_end, "elf"), None, "segment past the end");
    let mut no_exec = elf(20, &[MTSPR_HID4_R0]);
    put(&mut no_exec, 52 + 24, 4); // p_flags = R only
    assert_eq!(kind(&no_exec, "elf"), None, "no executable segment");
    assert_eq!(kind(b"\x7FELF", "elf"), None, "short header");
}

#[test]
fn path_reader_checks_the_extension() {
    let dir = tempfile::tempdir().unwrap();
    let image = dol(&[BLR, MTSPR_HID4_R0], &[]);
    let wii = crate::test_images::write(dir.path(), "boot.dol", &image);
    assert_eq!(read_executable_kind(&wii).unwrap(), WII);
    let other = crate::test_images::write(dir.path(), "boot.bin", &image);
    assert_eq!(read_executable_kind(&other).unwrap(), None);
}
