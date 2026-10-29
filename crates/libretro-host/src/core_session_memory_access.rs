//! Raw core memory of a running session: the `SET_MEMORY_MAPS` copy and
//! `retro_get_memory_data/size`. Achievements read the console's memory through
//! these (rcheevos `rc_libretro`); battery SRAM / RTC / system RAM slices go through
//! the same lookup.

use std::ffi::c_uint;

use crate::core_session::CoreSession;
use crate::memory_map_snapshot::MemoryMapSnapshot;

impl CoreSession {
    /// The core's latest `SET_MEMORY_MAPS` (deep copy); `None` if it never sent one.
    /// Compare [`MemoryMapSnapshot::generation`] to notice a re-sent map.
    pub fn memory_maps(&self) -> Option<&MemoryMapSnapshot> {
        self.ctx().memory_maps.as_ref()
    }

    /// `retro_get_memory_data/size(id)` (`RETRO_MEMORY_*`); `None` when the core
    /// reports no buffer or size 0. The pointer stays owned by the core: valid while
    /// the game is loaded, and the buffer may move or resize between frames (re-query).
    pub fn memory_info(&self, id: c_uint) -> Option<(*mut u8, usize)> {
        // SAFETY: plain queries on the loaded core; nothing is dereferenced here.
        let (data, size) = unsafe {
            (
                (self.core.get_memory_data)(id),
                (self.core.get_memory_size)(id),
            )
        };
        (!data.is_null() && size != 0).then_some((data.cast::<u8>(), size))
    }

    /// [`Self::memory_info`] as a slice, exclusively borrowed through `&mut self`.
    pub(crate) fn memory_mut(&mut self, id: c_uint) -> Option<&mut [u8]> {
        let (data, size) = self.memory_info(id)?;
        // SAFETY: `size` bytes at `data`, owned by the core for the session; no other
        // reference exists while `&mut self` is held.
        Some(unsafe { std::slice::from_raw_parts_mut(data, size) })
    }
}
