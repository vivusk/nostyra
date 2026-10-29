//! Disc control through a fake core callback table, plus the environment side
//! (EXT / legacy registration, `GET_VARIABLE`, core messages).

use std::ffi::{c_char, c_uint, c_void, CString};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use super::*;
use crate::environment_callback::environment;
use crate::ffi_disc_message_types::*;
use crate::ffi_types::{RETRO_ENVIRONMENT_GET_VARIABLE, RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE};
use crate::host_context::run_with_test_ctx;

/// Fake core state: inserted disc, tray, and the call log of the last swap.
static INDEX: AtomicU32 = AtomicU32::new(0);
static CALLS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Tests share the fake core: run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

unsafe extern "C" fn set_eject(ejected: bool) -> bool {
    CALLS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(format!("eject {ejected}"));
    true
}
unsafe extern "C" fn get_eject() -> bool {
    false
}
unsafe extern "C" fn get_index() -> c_uint {
    INDEX.load(Ordering::SeqCst)
}
unsafe extern "C" fn set_index(index: c_uint) -> bool {
    CALLS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(format!("index {index}"));
    INDEX.store(index, Ordering::SeqCst);
    index <= 2
}
unsafe extern "C" fn num_images() -> c_uint {
    2
}
unsafe extern "C" fn label(index: c_uint, out: *mut c_char, len: usize) -> bool {
    let text = match index {
        0 => CString::new("E:/PSX/Game (Disc 1)").unwrap_or_default(),
        _ => return false,
    };
    let bytes = text.as_bytes_with_nul();
    // SAFETY: test buffer of `len` bytes from DiscTable::text.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr().cast(), out, bytes.len().min(len)) };
    true
}
unsafe extern "C" fn path(index: c_uint, out: *mut c_char, len: usize) -> bool {
    let text = CString::new(format!("E:/PSX/Game (Disc {}).cue", index + 1)).unwrap_or_default();
    let bytes = text.as_bytes_with_nul();
    // SAFETY: as `label`.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr().cast(), out, bytes.len().min(len)) };
    true
}

fn legacy() -> RetroDiskControlCallback {
    RetroDiskControlCallback {
        set_eject_state: Some(set_eject),
        get_eject_state: Some(get_eject),
        get_image_index: Some(get_index),
        set_image_index: Some(set_index),
        get_num_images: Some(num_images),
        replace_image_index: None,
        add_image_index: None,
    }
}

fn ext() -> RetroDiskControlExtCallback {
    RetroDiskControlExtCallback {
        get_image_path: Some(path),
        get_image_label: Some(label),
        ..legacy().into()
    }
}

fn env(cmd: c_uint, data: *mut c_void) -> bool {
    // SAFETY: tests pass a pointer of the type documented for `cmd`.
    unsafe { environment(cmd, data) }
}

#[test]
fn swap_runs_eject_select_close_and_labels_fall_back() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    INDEX.store(0, Ordering::SeqCst);
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    let table = DiscTable(ext());
    assert_eq!((table.count(), table.index()), (2, 0));
    assert_eq!(
        table.label(0),
        "Game (Disc 1)",
        "label reduced to its file part"
    );
    assert_eq!(table.label(1), "Game (Disc 2)", "no label: path file stem");
    table.swap_to(1).unwrap();
    assert_eq!(table.index(), 1);
    assert_eq!(
        *CALLS.lock().unwrap_or_else(|e| e.into_inner()),
        ["eject true", "index 1", "eject false"]
    );
    CALLS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    assert!(matches!(
        table.swap_to(5),
        Err(HostError::DiscSwapRejected(5))
    ));
    assert_eq!(
        *CALLS.lock().unwrap_or_else(|e| e.into_inner()),
        ["eject true", "index 5", "index 1", "eject false"],
        "rejected swap restores the disc and closes the tray"
    );
    let bare = DiscTable(legacy().into());
    assert_eq!(bare.label(1), "Disc 2");
}

#[test]
fn environment_registers_tables_and_reports_version_1() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            let mut version: c_uint = 0;
            assert!(env(
                RETRO_ENVIRONMENT_GET_DISK_CONTROL_INTERFACE_VERSION,
                (&raw mut version).cast()
            ));
            assert_eq!(version, 1);
            let mut table = legacy();
            assert!(env(
                RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE,
                (&raw mut table).cast()
            ));
        },
    );
    let registered = ctx.disk_control.expect("legacy table kept");
    assert!(registered.get_image_label.is_none());
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            let mut table = ext();
            assert!(env(
                RETRO_ENVIRONMENT_SET_DISK_CONTROL_EXT_INTERFACE,
                (&raw mut table).cast()
            ));
        },
    );
    assert!(ctx
        .disk_control
        .is_some_and(|t| t.get_image_label.is_some()));
}

#[test]
fn get_variable_answers_known_keys_with_stable_pointers() {
    let key = CString::new("pcsx_rearmed_memcard1").unwrap_or_default();
    let unknown = CString::new("pcsx_rearmed_drc").unwrap_or_default();
    run_with_test_ctx(
        |ctx| {
            ctx.core_options = vec![(
                CString::new("pcsx_rearmed_memcard1").unwrap_or_default(),
                CString::new("libretro").unwrap_or_default(),
            )];
        },
        || {
            let mut var = RetroVariable {
                key: key.as_ptr(),
                value: std::ptr::null(),
            };
            assert!(env(RETRO_ENVIRONMENT_GET_VARIABLE, (&raw mut var).cast()));
            let first = var.value;
            // SAFETY: host-owned CString, valid for the session.
            let value = unsafe { std::ffi::CStr::from_ptr(first) };
            assert_eq!(value.to_str(), Ok("libretro"));
            assert!(env(RETRO_ENVIRONMENT_GET_VARIABLE, (&raw mut var).cast()));
            assert_eq!(var.value, first, "same pointer on every call");
            let mut other = RetroVariable {
                key: unknown.as_ptr(),
                value: std::ptr::null(),
            };
            assert!(!env(
                RETRO_ENVIRONMENT_GET_VARIABLE,
                (&raw mut other).cast()
            ));
            let mut update = true;
            assert!(!env(
                RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE,
                (&raw mut update).cast()
            ));
        },
    );
}

#[test]
fn core_messages_are_queued_unless_log_only() {
    let shown = CString::new("No PlayStation BIOS file found").unwrap_or_default();
    let log_only = CString::new("internal detail").unwrap_or_default();
    let ctx = run_with_test_ctx(
        |_| {},
        || {
            let mut ext = RetroMessageExt {
                msg: shown.as_ptr(),
                duration: 3000,
                priority: 1,
                level: 2,
                target: 0,
                kind: 0,
                progress: -1,
            };
            assert!(env(
                RETRO_ENVIRONMENT_SET_MESSAGE_EXT,
                (&raw mut ext).cast()
            ));
            ext.msg = log_only.as_ptr();
            ext.target = RETRO_MESSAGE_TARGET_LOG;
            assert!(env(
                RETRO_ENVIRONMENT_SET_MESSAGE_EXT,
                (&raw mut ext).cast()
            ));
            let mut legacy = RetroMessage {
                msg: shown.as_ptr(),
                frames: 60,
            };
            assert!(env(RETRO_ENVIRONMENT_SET_MESSAGE, (&raw mut legacy).cast()));
        },
    );
    assert_eq!(
        ctx.core_messages,
        [
            "No PlayStation BIOS file found",
            "No PlayStation BIOS file found"
        ]
    );
}
