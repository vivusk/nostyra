//! Owner of one `rc_libretro_memory_regions_t`: rcheevos' map from RetroAchievements
//! addresses to core memory, built from a core's `SET_MEMORY_MAPS` descriptors or, if
//! there are none, from `retro_get_memory_data` (SYSTEM_RAM / SAVE_RAM).
//!
//! `rc_libretro_memory_init`'s info callback has no userdata, so the caller's lookup
//! is lent to [`core_memory_info`] through a thread-local for the duration of that one
//! C call ([`InfoScope`] clears it on return and on unwind).
//!
//! Region pointers point into core memory: they are valid while the game stays
//! loaded and the core keeps its buffers (the caller rebuilds on a map re-send or a
//! buffer change and drops the regions before unloading the core).

use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};

use libretro_host::RetroMemoryDescriptor;

use super::libretro_ffi::*;

/// `retro_get_memory_data/size(id)` lookup: `None` when the core has no such buffer.
pub type CoreMemoryLookup<'a> = dyn Fn(u32) -> Option<(*mut u8, usize)> + 'a;

/// Memory ids the info callback serves: SAVE_RAM (0) and SYSTEM_RAM (2). VIDEO_RAM
/// (3) is refused: mGBA reports 0x18000 bytes for a 0x4000-byte GB VRAM (M15 D6).
const SERVED_IDS: [u32; 2] = [0, 2];

thread_local! {
    static LOOKUP: Cell<Option<*const CoreMemoryLookup<'static>>> = const { Cell::new(None) };
}

/// Lends a lookup to the trampoline; restores the previous value when dropped.
struct InfoScope(Option<*const CoreMemoryLookup<'static>>);

impl InfoScope {
    fn enter(lookup: &CoreMemoryLookup<'_>) -> Self {
        let ptr: *const CoreMemoryLookup<'_> = lookup;
        // SAFETY: only the trait object's lifetime is erased; the pointer is read only
        // by `core_memory_info` during the C call this scope encloses, while `lookup`
        // is still borrowed, and the scope's Drop removes it before the borrow ends.
        let ptr = unsafe {
            std::mem::transmute::<*const CoreMemoryLookup<'_>, *const CoreMemoryLookup<'static>>(
                ptr,
            )
        };
        Self(LOOKUP.with(|cell| cell.replace(Some(ptr))))
    }
}

impl Drop for InfoScope {
    fn drop(&mut self) {
        LOOKUP.with(|cell| cell.set(self.0));
    }
}

/// Whether a lookup is lent right now (tests: cleared after every init).
#[cfg(test)]
pub fn lookup_installed() -> bool {
    LOOKUP.with(Cell::get).is_some()
}

/// `rc_libretro_get_core_memory_info_func`.
unsafe extern "C" fn core_memory_info(id: u32, info: *mut RcLibretroCoreMemoryInfo) {
    if info.is_null() {
        return;
    }
    let found = LOOKUP
        .with(Cell::get)
        .filter(|_| SERVED_IDS.contains(&id))
        .and_then(|lookup| {
            // SAFETY: installed by an `InfoScope` that is alive during this C call.
            let lookup = unsafe { &*lookup };
            catch_unwind(AssertUnwindSafe(|| lookup(id))).ok().flatten()
        });
    let (data, size) = found.unwrap_or((std::ptr::null_mut(), 0));
    // SAFETY: rcheevos passes a valid out-param.
    unsafe {
        (*info).data = data;
        (*info).size = size;
    }
}

pub struct RawMemoryRegions {
    regions: Box<RcLibretroMemoryRegions>,
    has_memory: bool,
}

impl RawMemoryRegions {
    /// `rc_libretro_memory_init(console_id)` over `descriptors` (`None` or empty: the
    /// SYSTEM_RAM / SAVE_RAM layout from `lookup`).
    pub fn init(
        console_id: u32,
        descriptors: Option<&[RetroMemoryDescriptor]>,
        lookup: &CoreMemoryLookup<'_>,
    ) -> Self {
        let map = descriptors
            .filter(|d| !d.is_empty())
            .map(|d| RetroMemoryMap {
                descriptors: d.as_ptr(),
                num_descriptors: d.len() as u32,
            });
        let map_ptr = map.as_ref().map_or(std::ptr::null(), std::ptr::from_ref);
        // SAFETY: all-zero is a valid `rc_libretro_memory_regions_t` (null pointers,
        // zero sizes); rcheevos overwrites it.
        let mut regions: Box<RcLibretroMemoryRegions> = Box::new(unsafe { std::mem::zeroed() });
        let scope = InfoScope::enter(lookup);
        // SAFETY: valid out-param; `map_ptr` is null or points at `map`, whose
        // descriptors outlive the call; the trampoline reads the scoped lookup only.
        let result = unsafe {
            rc_libretro_memory_init(&mut *regions, map_ptr, core_memory_info, console_id)
        };
        drop(scope);
        Self {
            regions,
            has_memory: result != 0,
        }
    }

    /// At least one region points at real memory.
    pub fn has_memory(&self) -> bool {
        self.has_memory
    }

    /// Size of the RetroAchievements address space covered (null filler included).
    pub fn total_size(&self) -> usize {
        self.regions.total_size
    }

    /// `rc_libretro_memory_read`: bytes copied into `buffer` (0 for unmapped or null
    /// regions; a read never spans past a null region).
    pub fn read(&self, address: u32, buffer: &mut [u8]) -> u32 {
        let len = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
        // SAFETY: `buffer` is writable for `len` bytes; region pointers point into
        // live core memory (lifecycle contract in the module docs).
        unsafe { rc_libretro_memory_read(&*self.regions, address, buffer.as_mut_ptr(), len) }
    }
}

impl Drop for RawMemoryRegions {
    fn drop(&mut self) {
        // SAFETY: initialised regions, destroyed once.
        unsafe { rc_libretro_memory_destroy(&mut *self.regions) };
    }
}
