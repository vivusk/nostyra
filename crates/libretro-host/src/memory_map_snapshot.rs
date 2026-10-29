//! The host's own copy of a core's `SET_MEMORY_MAPS` descriptors.
//!
//! Cores may free their descriptor array right after the call (libretro.h: "the
//! frontend must maintain its own copy"), so the array is deep-copied, `addrspace`
//! strings included. The descriptors' `ptr`s point into core memory and stay valid
//! while the game is loaded: handing them out is safe, dereferencing them is the
//! reader's job (rcheevos' `rc_libretro` inside the `achievements` FFI).

use std::ffi::{CStr, CString};

use crate::ffi_types::RetroMemoryDescriptor;

/// One `SET_MEMORY_MAPS` call, deep-copied. Not `Clone`: the copied `addrspace`
/// pointers point into this value's own strings.
#[derive(Debug)]
pub struct MemoryMapSnapshot {
    descriptors: Vec<RetroMemoryDescriptor>,
    /// Owners of the `addrspace` strings the copied descriptors point at (heap
    /// buffers: stable when the vectors move).
    _addrspaces: Vec<CString>,
    generation: u64,
}

impl MemoryMapSnapshot {
    /// Copies `src`, re-pointing each non-null `addrspace` at an owned copy.
    ///
    /// # Safety
    /// Every non-null `addrspace` in `src` is a NUL-terminated C string valid for the
    /// duration of the call.
    pub(crate) unsafe fn copy_from(src: &[RetroMemoryDescriptor], generation: u64) -> Self {
        let mut addrspaces = Vec::new();
        let descriptors = src
            .iter()
            .map(|desc| {
                let mut copy = *desc;
                if !desc.addrspace.is_null() {
                    // SAFETY: caller contract (NUL-terminated, valid during the call).
                    let owned = unsafe { CStr::from_ptr(desc.addrspace) }.to_owned();
                    copy.addrspace = owned.as_ptr();
                    addrspaces.push(owned);
                }
                copy
            })
            .collect();
        Self {
            descriptors,
            _addrspaces: addrspaces,
            generation,
        }
    }

    /// A map built by the frontend itself (tests, tools): `addrspace` names are
    /// dropped (set to NULL) since no owner exists for them.
    pub fn from_parts(mut descriptors: Vec<RetroMemoryDescriptor>, generation: u64) -> Self {
        descriptors
            .iter_mut()
            .for_each(|d| d.addrspace = std::ptr::null());
        Self {
            descriptors,
            _addrspaces: Vec::new(),
            generation,
        }
    }

    /// The copied descriptors, in the core's order (may be empty).
    pub fn descriptors(&self) -> &[RetroMemoryDescriptor] {
        &self.descriptors
    }

    /// 1 for the session's first `SET_MEMORY_MAPS`, +1 per later call: a reader built
    /// from an older generation must be rebuilt.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The `addrspace` name of descriptor `index`, if it has one.
    pub fn addrspace(&self, index: usize) -> Option<&str> {
        let desc = self.descriptors.get(index)?;
        if desc.addrspace.is_null() {
            return None;
        }
        // SAFETY: non-null `addrspace` points into `_addrspaces`, owned by `self`.
        unsafe { CStr::from_ptr(desc.addrspace) }.to_str().ok()
    }
}
