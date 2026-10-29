//! The lazy build policy: wait for memory, give up after the budget, follow changes.

use std::cell::Cell;

use super::*;

const SNES: u32 = 3;

/// SYSTEM_RAM lookup that appears from frame `from` on.
struct LateRam {
    ram: Vec<u8>,
    frame: Cell<u32>,
    from: u32,
}

impl LateRam {
    fn new(from: u32, fill: u8) -> Self {
        Self {
            ram: vec![fill; 0x2_0000],
            frame: Cell::new(0),
            from,
        }
    }

    fn lookup(&self, id: u32) -> Option<(*mut u8, usize)> {
        (id == 2 && self.frame.get() >= self.from)
            .then(|| (self.ram.as_ptr().cast_mut(), self.ram.len()))
    }
}

fn first_byte(lazy: &LazyMemoryRegions) -> Option<u8> {
    let mut b = [0u8; 1];
    (lazy.read(0, &mut b) == 1).then_some(b[0])
}

#[test]
fn waits_until_the_core_exposes_memory() {
    let late = LateRam::new(2, 0x5A);
    let mut lazy = LazyMemoryRegions::new(SNES);
    late.frame.set(1);
    assert_eq!(
        lazy.after_real_frame(None, &|id| late.lookup(id)),
        RegionStatus::Waiting
    );
    assert_eq!(first_byte(&lazy), None);
    late.frame.set(2);
    assert_eq!(
        lazy.after_real_frame(None, &|id| late.lookup(id)),
        RegionStatus::Ready
    );
    assert_eq!(first_byte(&lazy), Some(0x5A));
    assert_eq!(
        lazy.after_real_frame(None, &|id| late.lookup(id)),
        RegionStatus::Ready
    );
}

#[test]
fn gives_up_after_the_empty_frame_budget_until_reset() {
    let mut lazy = LazyMemoryRegions::new(SNES);
    for _ in 1..MAX_EMPTY_FRAMES {
        assert_eq!(
            lazy.after_real_frame(None, &|_| None),
            RegionStatus::Waiting
        );
    }
    assert_eq!(
        lazy.after_real_frame(None, &|_| None),
        RegionStatus::Unavailable
    );
    let late = LateRam::new(0, 1);
    assert_eq!(
        lazy.after_real_frame(None, &|id| late.lookup(id)),
        RegionStatus::Unavailable
    );
    lazy.reset();
    assert_eq!(
        lazy.after_real_frame(None, &|id| late.lookup(id)),
        RegionStatus::Ready
    );
}

#[test]
fn moved_buffer_is_followed_on_the_next_frame() {
    let (a, b) = (LateRam::new(0, 0xAA), LateRam::new(0, 0xBB));
    let mut lazy = LazyMemoryRegions::new(SNES);
    assert_eq!(
        lazy.after_real_frame(None, &|id| a.lookup(id)),
        RegionStatus::Ready
    );
    assert_eq!(first_byte(&lazy), Some(0xAA));
    assert_eq!(
        lazy.after_real_frame(None, &|id| b.lookup(id)),
        RegionStatus::Ready
    );
    assert_eq!(first_byte(&lazy), Some(0xBB));
}

#[test]
fn map_sent_during_a_frame_replaces_the_fallback() {
    use libretro_host::MemoryMapSnapshot;
    let mut ram = vec![0x77u8; 0x8000];
    let late = LateRam::new(0, 0x11);
    let mut lazy = LazyMemoryRegions::new(SNES);
    assert_eq!(
        lazy.after_real_frame(None, &|id| late.lookup(id)),
        RegionStatus::Ready
    );
    assert_eq!(lazy.regions().and_then(MemoryRegions::generation), None);
    let desc = libretro_host::RetroMemoryDescriptor {
        flags: 0,
        ptr: ram.as_mut_ptr().cast(),
        offset: 0,
        start: 0x7E_0000,
        select: 0,
        disconnect: 0,
        len: ram.len(),
        addrspace: std::ptr::null(),
    };
    let maps = MemoryMapSnapshot::from_parts(vec![desc], 1);
    assert_eq!(
        lazy.after_real_frame(Some(&maps), &|id| late.lookup(id)),
        RegionStatus::Ready
    );
    assert_eq!(lazy.regions().and_then(MemoryRegions::generation), Some(1));
    assert_eq!(first_byte(&lazy), Some(0x77));
}
