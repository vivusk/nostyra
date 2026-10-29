//! Fake-core checks of what melonDS DS needs from the session: a rejected
//! `retro_load_game` carries the core's reason (message first, else the last error
//! log line that is not the "Skipping error screen" notice), and `SAVE_RAM` is read
//! at its current pointer and size on every access (a state load may replace it).

use std::ffi::{c_uint, c_void};
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

use crate::core_library_loader::CoreLibrary;
use crate::core_session_hw_render::fake_core_tests::{fake_core, start_core};
use crate::environment_callback::environment;
use crate::environment_core_messages::log_line_for_tests;
use crate::ffi_disc_message_types::{RetroMessage, RETRO_ENVIRONMENT_SET_MESSAGE};
use crate::ffi_types::{RetroGameInfo, RETRO_MEMORY_SAVE_RAM};
use crate::host_error::HostError;
use crate::hw_render_test_fakes::{serial, FakeProvider};

const SKIP_NOTICE: &str =
    "Skipping error screen due to the environment variable MELONDSDS_SKIP_ERROR_SCREEN";

/// melonDS DS config error: error log lines only, the notice last.
unsafe extern "C" fn reject_with_log(_: *const RetroGameInfo) -> bool {
    log_line_for_tests("The loaded ROM is encrypted and needs native NDS BIOS files");
    log_line_for_tests(SKIP_NOTICE);
    false
}

/// melonDS DS bad ROM: an on-screen message (plus the log notice).
unsafe extern "C" fn reject_with_message(_: *const RetroGameInfo) -> bool {
    log_line_for_tests("an earlier error line");
    let mut message = RetroMessage {
        msg: c"ROM isn't valid, did you select the right file?".as_ptr(),
        frames: 60,
    };
    // SAFETY: `const struct retro_message *`, valid for the call.
    unsafe { environment(RETRO_ENVIRONMENT_SET_MESSAGE, (&raw mut message).cast()) };
    false
}

unsafe extern "C" fn reject_silently(_: *const RetroGameInfo) -> bool {
    log_line_for_tests(SKIP_NOTICE);
    false
}

fn rejection(load_game: unsafe extern "C" fn(*const RetroGameInfo) -> bool) -> HostError {
    let core = CoreLibrary {
        load_game,
        ..fake_core()
    };
    match start_core(core, &FakeProvider::default()) {
        Err(error) => error,
        Ok(_) => panic!("the fake core rejects the game"),
    }
}

#[test]
fn rejected_load_carries_the_cores_reason() {
    let _serial = serial();
    let error = rejection(reject_with_log);
    assert!(matches!(
        &error,
        HostError::LoadGameRejected(Some(reason)) if reason.starts_with("The loaded ROM is encrypted")
    ));
    assert_eq!(
        error.to_string(),
        "core rejected the game (retro_load_game returned false): \
         The loaded ROM is encrypted and needs native NDS BIOS files"
    );
    let error = rejection(reject_with_message);
    assert!(matches!(
        &error,
        HostError::LoadGameRejected(Some(reason)) if reason.starts_with("ROM isn't valid")
    ));
    let error = rejection(reject_silently);
    assert!(
        matches!(error, HostError::LoadGameRejected(None)),
        "{error}"
    );
    assert_eq!(
        error.to_string(),
        "core rejected the game (retro_load_game returned false)"
    );
}

static SRAM: [AtomicU8; 16] = [const { AtomicU8::new(0) }; 16];
/// Where the fake SAVE_RAM starts in `SRAM` and how long it is (changed by the test).
static SRAM_AT: AtomicUsize = AtomicUsize::new(0);
static SRAM_LEN: AtomicUsize = AtomicUsize::new(8);

unsafe extern "C" fn sram_data(id: c_uint) -> *mut c_void {
    match id {
        RETRO_MEMORY_SAVE_RAM => SRAM[SRAM_AT.load(Ordering::SeqCst)].as_ptr().cast(),
        _ => std::ptr::null_mut(),
    }
}

unsafe extern "C" fn sram_size(id: c_uint) -> usize {
    match id {
        RETRO_MEMORY_SAVE_RAM => SRAM_LEN.load(Ordering::SeqCst),
        _ => 0,
    }
}

#[test]
fn save_ram_is_read_at_its_current_pointer_and_size() {
    let _serial = serial();
    let core = CoreLibrary {
        get_memory_data: sram_data,
        get_memory_size: sram_size,
        ..fake_core()
    };
    let mut session = start_core(core, &FakeProvider::default()).expect("session loads");
    assert_eq!(session.save_ram_mut().map(|s| s.len()), Some(8));
    session.save_ram_mut().unwrap()[0] = 0xAB;
    assert_eq!(SRAM[0].load(Ordering::SeqCst), 0xAB);
    // "A state load re-allocates the buffer": new place, new size.
    SRAM_AT.store(4, Ordering::SeqCst);
    SRAM_LEN.store(12, Ordering::SeqCst);
    let moved = session.save_ram_mut().unwrap();
    assert_eq!(moved.len(), 12);
    moved[0] = 0xCD;
    assert_eq!(SRAM[4].load(Ordering::SeqCst), 0xCD);
    SRAM_LEN.store(0, Ordering::SeqCst); // homebrew: no SAVE_RAM → no save.srm
    assert!(session.save_ram_mut().is_none());
    drop(session);
    SRAM_AT.store(0, Ordering::SeqCst);
    SRAM_LEN.store(8, Ordering::SeqCst);
}
