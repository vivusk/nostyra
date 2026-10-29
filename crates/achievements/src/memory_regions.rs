//! RetroAchievements addresses → core memory (rcheevos `rc_libretro`), and the facts
//! that decide when the mapping must be rebuilt.
//!
//! Built from the core's `SET_MEMORY_MAPS` copy when there is one, else from
//! SAVE_RAM / SYSTEM_RAM (`retro_get_memory_data`). Game Boy / Color / Advance never
//! use that fallback: mGBA's fallback buffers sit at the wrong RA addresses (M15 D6).
//! Lifecycle contract (the regions hold raw core pointers): build only after the
//! first real frame, rebuild when [`MemoryRegions::is_stale`], drop before the core
//! unloads. Bytes are read raw (never byte-swapped).

use libretro_host::MemoryMapSnapshot;

pub use crate::ffi::CoreMemoryLookup;
use crate::ffi::RawMemoryRegions;

/// `RETRO_MEMORY_SAVE_RAM` / `RETRO_MEMORY_SYSTEM_RAM`.
const SAVE_RAM: u32 = 0;
const SYSTEM_RAM: u32 = 2;
/// RA console ids that need the core's memory map: Game Boy (4), Game Boy Advance
/// (5), Game Boy Color (6).
const MAP_REQUIRED_CONSOLES: [u32; 3] = [4, 5, 6];

/// What a set of regions was built from: map generation and the ids 0 / 2 buffers.
/// GPGX shrinks SAVE_RAM after frame 1, mGBA sends its map during frame 1, so any
/// change means a rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MemorySource {
    map_generation: Option<u64>,
    save_ram: Option<(usize, usize)>,
    system_ram: Option<(usize, usize)>,
}

impl MemorySource {
    fn of(maps: Option<&MemoryMapSnapshot>, lookup: &CoreMemoryLookup<'_>) -> Self {
        let buffer = |id| lookup(id).map(|(ptr, size)| (ptr as usize, size));
        Self {
            map_generation: maps.map(MemoryMapSnapshot::generation),
            save_ram: buffer(SAVE_RAM),
            system_ram: buffer(SYSTEM_RAM),
        }
    }
}

/// One `rc_libretro` mapping for one console.
pub struct MemoryRegions {
    raw: Option<RawMemoryRegions>,
    source: MemorySource,
}

impl MemoryRegions {
    /// Builds the mapping for `console_id` (RA id). `lookup` answers
    /// `retro_get_memory_data/size` (only ids 0 and 2 are ever asked).
    pub fn init(
        console_id: u32,
        maps: Option<&MemoryMapSnapshot>,
        lookup: &CoreMemoryLookup<'_>,
    ) -> Self {
        let source = MemorySource::of(maps, lookup);
        let descriptors = maps.map(MemoryMapSnapshot::descriptors);
        let has_map = descriptors.is_some_and(|d| !d.is_empty());
        let raw = (has_map || !MAP_REQUIRED_CONSOLES.contains(&console_id))
            .then(|| RawMemoryRegions::init(console_id, descriptors, lookup));
        Self { raw, source }
    }

    /// No region points at real memory: every read returns 0 bytes.
    pub fn is_empty(&self) -> bool {
        !self.raw.as_ref().is_some_and(RawMemoryRegions::has_memory)
    }

    /// Generation of the memory map this mapping came from (`None`: no map).
    pub fn generation(&self) -> Option<u64> {
        self.source.map_generation
    }

    /// The core re-sent its map or moved / resized SAVE_RAM or SYSTEM_RAM since this
    /// mapping was built: rebuild before the next read.
    pub fn is_stale(
        &self,
        maps: Option<&MemoryMapSnapshot>,
        lookup: &CoreMemoryLookup<'_>,
    ) -> bool {
        MemorySource::of(maps, lookup) != self.source
    }

    /// Size of the RA address space covered (null filler included).
    pub fn total_size(&self) -> usize {
        self.raw.as_ref().map_or(0, RawMemoryRegions::total_size)
    }

    /// Copies `buffer.len()` bytes from RA address `address`; returns the bytes read
    /// (0 when unmapped or empty).
    pub fn read(&self, address: u32, buffer: &mut [u8]) -> u32 {
        self.raw.as_ref().map_or(0, |raw| raw.read(address, buffer))
    }
}

#[cfg(test)]
#[path = "memory_regions_tests.rs"]
mod tests;
