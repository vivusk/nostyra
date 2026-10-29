//! Synthetic memory maps / buffers: RA addresses land on the right bytes per console,
//! the GB family never falls back, and a changed source asks for a rebuild.

use std::cell::RefCell;
use std::ffi::c_void;

use libretro_host::{MemoryMapSnapshot, RetroMemoryDescriptor};

use super::*;
use crate::ffi::lookup_installed;

const GBA: u32 = 5;
const GB: u32 = 4;
const SNES: u32 = 3;
const PS1: u32 = 12;
const NDS: u32 = 18;

/// Buffer whose byte `i` is `tag ^ i` (low byte), so every read names its source.
fn patterned(len: usize, tag: u8) -> Vec<u8> {
    (0..len).map(|i| tag ^ (i as u8)).collect()
}

fn desc(buf: &mut [u8], start: usize, select: usize, disconnect: usize) -> RetroMemoryDescriptor {
    RetroMemoryDescriptor {
        flags: 0,
        ptr: buf.as_mut_ptr().cast::<c_void>(),
        offset: 0,
        start,
        select,
        disconnect,
        len: buf.len(),
        addrspace: std::ptr::null(),
    }
}

fn none(_: u32) -> Option<(*mut u8, usize)> {
    None
}

fn byte(regions: &MemoryRegions, address: u32) -> Option<u8> {
    let mut b = [0u8; 1];
    (regions.read(address, &mut b) == 1).then_some(b[0])
}

#[test]
fn gba_map_puts_iwram_ewram_and_sram_at_ra_addresses() {
    let (mut iwram, mut ewram, mut sram) = (
        patterned(0x8000, 0x11),
        patterned(0x4_0000, 0x22),
        patterned(0x1_0000, 0x33),
    );
    let maps = MemoryMapSnapshot::from_parts(
        vec![
            desc(&mut iwram, 0x0300_0000, 0, 0),
            desc(&mut ewram, 0x0200_0000, 0, 0),
            desc(&mut sram, 0x0E00_0000, 0, 0),
        ],
        1,
    );
    let regions = MemoryRegions::init(GBA, Some(&maps), &none);
    assert!(!regions.is_empty());
    assert_eq!(regions.generation(), Some(1));
    assert_eq!(byte(&regions, 0x0005), Some(0x11 ^ 5));
    assert_eq!(byte(&regions, 0x8000 + 0x10), Some(0x22 ^ 0x10));
    assert_eq!(byte(&regions, 0x4_8001), Some(0x33 ^ 1));
    let mut word = [0u8; 4];
    assert_eq!(
        regions.read(0x7FFE, &mut word),
        4,
        "reads span IWRAM → EWRAM"
    );
    assert_eq!(word, [0x11 ^ 0xFE, 0x11 ^ 0xFF, 0x22, 0x23]);
    assert!(!lookup_installed(), "TLS lookup cleared after init");
}

#[test]
fn nds_map_reaches_main_ram_and_the_dtcm_pseudo_address() {
    let (mut main, mut dtcm) = (patterned(0x40_0000, 0x44), patterned(0x4000, 0x55));
    let maps = MemoryMapSnapshot::from_parts(
        vec![
            desc(&mut main, 0x0200_0000, 0xFF00_0000, 0xC0_0000),
            desc(&mut dtcm, 0x0E00_0000, 0, 0),
        ],
        1,
    );
    let regions = MemoryRegions::init(NDS, Some(&maps), &none);
    assert_eq!(byte(&regions, 0x123), Some(0x44 ^ 0x23));
    assert_eq!(
        byte(&regions, 0x100_0002),
        Some(0x55 ^ 2),
        "DTCM at RA $1000000"
    );
}

#[test]
fn ps1_map_covers_ram_and_scratchpad() {
    let (mut ram, mut scratch) = (patterned(0x20_0000, 0x66), patterned(0x400, 0x77));
    let maps = MemoryMapSnapshot::from_parts(
        vec![
            desc(&mut ram, 0, 0x5FE0_0000, 0),
            desc(&mut scratch, 0x1F80_0000, 0, 0),
        ],
        3,
    );
    let regions = MemoryRegions::init(PS1, Some(&maps), &none);
    assert_eq!(byte(&regions, 0x1_0005), Some(0x66 ^ 5));
    assert_eq!(byte(&regions, 0x20_0001), Some(0x77 ^ 1));
    assert_eq!(regions.generation(), Some(3));
}

#[test]
fn snes_without_map_falls_back_to_system_and_save_ram() {
    let (mut wram, mut sram) = (patterned(0x2_0000, 0x12), patterned(0x800, 0x34));
    let (w, s) = (
        (wram.as_mut_ptr(), wram.len()),
        (sram.as_mut_ptr(), sram.len()),
    );
    let lookup = move |id| match id {
        2 => Some(w),
        0 => Some(s),
        _ => None,
    };
    let regions = MemoryRegions::init(SNES, None, &lookup);
    assert_eq!(regions.generation(), None);
    assert_eq!(byte(&regions, 0x05), Some(0x12 ^ 5));
    assert_eq!(byte(&regions, 0x2_0003), Some(0x34 ^ 3));
    assert_eq!(byte(&regions, 0x2_0900), None, "past SRAM = null filler");
}

#[test]
fn game_boy_family_never_uses_the_fallback() {
    let mut ram = patterned(0x8000, 0x01);
    let buf = (ram.as_mut_ptr(), ram.len());
    let asked = RefCell::new(Vec::new());
    let lookup = |id| {
        asked.borrow_mut().push(id);
        Some(buf)
    };
    for console in [4, 5, 6] {
        let regions = MemoryRegions::init(console, None, &lookup);
        assert!(regions.is_empty(), "console {console}");
        assert_eq!(byte(&regions, 0xC000), None);
    }
    let empty_map = MemoryMapSnapshot::from_parts(Vec::new(), 1);
    assert!(MemoryRegions::init(GB, Some(&empty_map), &lookup).is_empty());
    assert!(
        asked.borrow().iter().all(|id| *id == 0 || *id == 2),
        "only ids 0/2 asked"
    );
}

#[test]
fn re_sent_map_or_moved_buffer_makes_regions_stale() {
    let (mut a, mut b) = (patterned(0x8000, 0xA0), patterned(0x8000, 0xB0));
    let first = MemoryMapSnapshot::from_parts(vec![desc(&mut a, 0x0300_0000, 0, 0)], 1);
    let regions = MemoryRegions::init(GBA, Some(&first), &none);
    assert!(!regions.is_stale(Some(&first), &none));
    let second = MemoryMapSnapshot::from_parts(vec![desc(&mut b, 0x0300_0000, 0, 0)], 2);
    assert!(regions.is_stale(Some(&second), &none));
    let rebuilt = MemoryRegions::init(GBA, Some(&second), &none);
    assert_eq!(rebuilt.generation(), Some(2));
    assert_eq!(
        byte(&rebuilt, 0x0001),
        Some(0xB0 ^ 1),
        "reads follow the new map"
    );

    let mut save = patterned(0x10, 0);
    let shrunk = (save.as_mut_ptr(), 2usize);
    let full = (save.as_mut_ptr(), 0x10usize);
    let fallback = MemoryRegions::init(SNES, None, &move |id| (id == 0).then_some(full));
    assert!(
        fallback.is_stale(None, &move |id| (id == 0).then_some(shrunk)),
        "SAVE_RAM size change"
    );
}

#[test]
fn info_callback_serves_only_ids_0_and_2_and_survives_a_panicking_lookup() {
    let asked = RefCell::new(Vec::new());
    let recording = |id| {
        asked.borrow_mut().push(id);
        None
    };
    // The GB fallback layout asks VIDEO_RAM for tile RAM: refused before the lookup.
    let raw = crate::ffi::RawMemoryRegions::init(GB, None, &recording);
    assert!(!raw.has_memory());
    assert!(!asked.borrow().is_empty());
    assert!(asked.borrow().iter().all(|id| *id == 0 || *id == 2));
    let panicking = |_: u32| -> Option<(*mut u8, usize)> { panic!("lookup bug") };
    let raw = crate::ffi::RawMemoryRegions::init(SNES, None, &panicking);
    assert!(!raw.has_memory());
    assert!(!lookup_installed());
}
