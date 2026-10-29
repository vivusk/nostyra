//! `RETRO_ENVIRONMENT_SET_MEMORY_MAPS` (36) and `SET_SUPPORT_ACHIEVEMENTS` (42).
//!
//! Env 36 deep-copies the core's descriptors into `HostContext.memory_maps`; a later
//! call (mGBA sends its map inside the first `retro_run`, GPGX Sega CD / PCSX HLE
//! re-send) replaces the copy and bumps its generation. Env 42 is accepted and only
//! logged: just two pinned cores send it, so nothing may depend on it (M15 spike D6).

use std::ffi::{c_uint, c_void};

use crate::ffi_types::{RetroMemoryDescriptor, RetroMemoryMap};
use crate::host_context::HostContext;
use crate::memory_map_snapshot::MemoryMapSnapshot;

pub(crate) const RETRO_ENVIRONMENT_SET_MEMORY_MAPS: c_uint = 36;
pub(crate) const RETRO_ENVIRONMENT_SET_SUPPORT_ACHIEVEMENTS: c_uint = 42;

/// # Safety
/// `data` must be the type libretro.h documents for `cmd` (or null).
pub(crate) unsafe fn memory_maps(ctx: &mut HostContext, cmd: c_uint, data: *mut c_void) -> bool {
    if cmd == RETRO_ENVIRONMENT_SET_SUPPORT_ACHIEVEMENTS {
        if data.is_null() {
            return false;
        }
        // SAFETY: `const bool *` for this command.
        let supported = unsafe { *data.cast::<bool>() };
        tracing::debug!(supported, "core reports achievement support");
        return true;
    }
    let map = data.cast::<RetroMemoryMap>();
    if map.is_null() {
        return false;
    }
    // SAFETY: `const struct retro_memory_map *` holding `num_descriptors` descriptors
    // (or a null array), valid for the duration of the call.
    let src: &[RetroMemoryDescriptor] = unsafe {
        let map = &*map;
        if map.descriptors.is_null() || map.num_descriptors == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(map.descriptors, map.num_descriptors as usize)
        }
    };
    let generation = ctx.memory_maps.as_ref().map_or(0, |m| m.generation()) + 1;
    // SAFETY: the core's `addrspace` strings are valid C strings during the call.
    let snapshot = unsafe { MemoryMapSnapshot::copy_from(src, generation) };
    tracing::debug!(
        descriptors = src.len(),
        generation,
        "core memory map received"
    );
    ctx.memory_maps = Some(snapshot);
    true
}

#[cfg(test)]
#[path = "environment_memory_maps_tests.rs"]
mod tests;
