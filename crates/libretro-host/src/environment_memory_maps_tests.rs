//! Env 36 / 42 through the real `retro_environment` entry point.

use std::ffi::{c_uint, c_void, CString};

use super::*;
use crate::environment_callback::environment;
use crate::ffi_types::RETRO_ENVIRONMENT_EXPERIMENTAL;
use crate::host_context::run_with_test_ctx;

fn env(cmd: c_uint, data: *mut c_void) -> bool {
    // SAFETY: tests pass a pointer of the type documented for `cmd`.
    unsafe { environment(cmd | RETRO_ENVIRONMENT_EXPERIMENTAL, data) }
}

fn descriptor(start: usize, len: usize, ptr: *mut c_void) -> RetroMemoryDescriptor {
    RetroMemoryDescriptor {
        flags: 0,
        ptr,
        offset: 0,
        start,
        select: 0,
        disconnect: 0,
        len,
        addrspace: std::ptr::null(),
    }
}

/// Sends `descs` like a core whose array (and strings) are freed right after the call.
fn send_and_free(mut descs: Vec<RetroMemoryDescriptor>, names: Vec<CString>) {
    let mut map = RetroMemoryMap {
        descriptors: descs.as_ptr(),
        num_descriptors: descs.len() as c_uint,
    };
    assert!(env(
        RETRO_ENVIRONMENT_SET_MEMORY_MAPS,
        (&raw mut map).cast()
    ));
    // Scribble over the core-owned copy before freeing it.
    descs
        .iter_mut()
        .for_each(|d| *d = descriptor(0xDEAD, 0, std::ptr::null_mut()));
    drop(descs);
    drop(names);
}

#[test]
fn memory_map_is_deep_copied_and_survives_the_core_freeing_it() {
    let mut ram = [0u8; 16];
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            let name = CString::new("IWRAM").unwrap();
            let mut first = descriptor(0x0300_0000, 0x8000, ram.as_mut_ptr().cast());
            first.addrspace = name.as_ptr();
            send_and_free(
                vec![
                    first,
                    descriptor(0x0200_0000, 0x4_0000, std::ptr::null_mut()),
                ],
                vec![name],
            );
        },
    );
    let maps = ctx.memory_maps.as_ref().expect("snapshot stored");
    assert_eq!(maps.generation(), 1);
    assert_eq!(maps.descriptors().len(), 2);
    assert_eq!(maps.descriptors()[0].start, 0x0300_0000);
    assert_eq!(maps.descriptors()[0].ptr, ram.as_mut_ptr().cast());
    assert_eq!(maps.addrspace(0), Some("IWRAM"));
    assert_eq!(maps.addrspace(1), None);
    assert_eq!(maps.descriptors()[1].len, 0x4_0000);
}

#[test]
fn re_sent_map_replaces_the_copy_and_bumps_the_generation() {
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            send_and_free(vec![descriptor(0, 0x800, std::ptr::null_mut())], vec![]);
            send_and_free(
                vec![
                    descriptor(0x1000, 0x10, std::ptr::null_mut()),
                    descriptor(0x2000, 0x20, std::ptr::null_mut()),
                ],
                vec![],
            );
        },
    );
    let maps = ctx.memory_maps.as_ref().expect("snapshot stored");
    assert_eq!(maps.generation(), 2);
    let starts: Vec<usize> = maps.descriptors().iter().map(|d| d.start).collect();
    assert_eq!(starts, [0x1000, 0x2000]);
}

#[test]
fn empty_map_is_stored_and_null_is_refused() {
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            let mut map = RetroMemoryMap {
                descriptors: std::ptr::null(),
                num_descriptors: 3,
            };
            assert!(env(
                RETRO_ENVIRONMENT_SET_MEMORY_MAPS,
                (&raw mut map).cast()
            ));
            assert!(!env(
                RETRO_ENVIRONMENT_SET_MEMORY_MAPS,
                std::ptr::null_mut()
            ));
        },
    );
    let maps = ctx.memory_maps.as_ref().expect("snapshot stored");
    assert!(maps.descriptors().is_empty());
    assert_eq!(maps.generation(), 1);
}

#[test]
fn support_achievements_is_accepted_without_state() {
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            let mut yes = true;
            assert!(env(
                RETRO_ENVIRONMENT_SET_SUPPORT_ACHIEVEMENTS,
                (&raw mut yes).cast()
            ));
            assert!(!env(
                RETRO_ENVIRONMENT_SET_SUPPORT_ACHIEVEMENTS,
                std::ptr::null_mut()
            ));
        },
    );
    assert!(ctx.memory_maps.is_none());
}
