//! When to (re)build [`MemoryRegions`] during a session (M15 D6): never before the
//! first real frame (mGBA sends its map inside frame 1, PPSSPP exposes RAM from frame
//! 2), rebuild whenever the source changed, and give up after
//! [`MAX_EMPTY_FRAMES`] frames without any mapped memory.

use libretro_host::MemoryMapSnapshot;

use crate::memory_regions::{CoreMemoryLookup, MemoryRegions};

/// Real frames without mapped memory before achievements are declared unavailable
/// for this core / game.
pub const MAX_EMPTY_FRAMES: u32 = 300;

/// Result of [`LazyMemoryRegions::after_real_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionStatus {
    /// Reads hit core memory.
    Ready,
    /// No memory yet; reads return 0. Retried after the next frame.
    Waiting,
    /// Still nothing after [`MAX_EMPTY_FRAMES`]: unload the game set and tell the user.
    Unavailable,
}

/// Regions for one loaded game, rebuilt on demand.
pub struct LazyMemoryRegions {
    console_id: u32,
    regions: Option<MemoryRegions>,
    empty_frames: u32,
    unavailable: bool,
}

impl LazyMemoryRegions {
    pub fn new(console_id: u32) -> Self {
        Self {
            console_id,
            regions: None,
            empty_frames: 0,
            unavailable: false,
        }
    }

    /// Call after every real frame, before `rc_client_do_frame`: builds the regions
    /// the first time, rebuilds them when the core's map or buffers changed.
    pub fn after_real_frame(
        &mut self,
        maps: Option<&MemoryMapSnapshot>,
        lookup: &CoreMemoryLookup<'_>,
    ) -> RegionStatus {
        if self.unavailable {
            return RegionStatus::Unavailable;
        }
        let current = self
            .regions
            .as_ref()
            .filter(|r| !r.is_empty() && !r.is_stale(maps, lookup));
        if current.is_some() {
            return RegionStatus::Ready;
        }
        let regions = MemoryRegions::init(self.console_id, maps, lookup);
        let empty = regions.is_empty();
        self.regions = Some(regions);
        if !empty {
            self.empty_frames = 0;
            return RegionStatus::Ready;
        }
        self.empty_frames += 1;
        if self.empty_frames >= MAX_EMPTY_FRAMES {
            self.unavailable = true;
            self.regions = None;
            return RegionStatus::Unavailable;
        }
        RegionStatus::Waiting
    }

    /// Reads `buffer.len()` bytes at RA `address` (0 bytes while not ready).
    pub fn read(&self, address: u32, buffer: &mut [u8]) -> u32 {
        self.regions.as_ref().map_or(0, |r| r.read(address, buffer))
    }

    /// Drops the regions (before unloading the core, or on a game / disc change):
    /// the next frame starts over, including the empty-frame budget.
    pub fn reset(&mut self) {
        *self = Self::new(self.console_id);
    }

    /// The current regions, if built.
    pub fn regions(&self) -> Option<&MemoryRegions> {
        self.regions.as_ref()
    }
}

#[cfg(test)]
#[path = "lazy_memory_regions_tests.rs"]
mod tests;
