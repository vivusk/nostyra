//! Registry-flagged core rules against the fake core: no state call without a
//! hardware context, library kept mapped, abandoned sessions make no core call.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::core_library_loader::CoreLibrary;
use crate::core_session_hw_render::fake_core_tests::{fake_core, start, start_core};
use crate::environment_callback::environment;
use crate::ffi_hw_render_types::{
    RetroHwRenderCallback, RETRO_ENVIRONMENT_SET_HW_RENDER, RETRO_HW_CONTEXT_VULKAN,
};
use crate::ffi_types::RetroGameInfo;
use crate::host_error::HostResult;
use crate::hw_render_test_fakes::{log_take, serial, FakeProvider};
use crate::savestate_context::SavestateContext;

/// Every state entry point of the counting core below.
static STATE_CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn counted_size() -> usize {
    STATE_CALLS.fetch_add(1, Ordering::SeqCst);
    4
}
unsafe extern "C" fn counted_serialize(_: *mut c_void, _: usize) -> bool {
    STATE_CALLS.fetch_add(1, Ordering::SeqCst);
    true
}
unsafe extern "C" fn counted_unserialize(_: *const c_void, _: usize) -> bool {
    STATE_CALLS.fetch_add(1, Ordering::SeqCst);
    true
}
/// Loads without asking for a graphics context (Dolphin on `renderer=Null`).
unsafe extern "C" fn software_load(_: *const RetroGameInfo) -> bool {
    true
}
/// Asks for Vulkan only (refused by `FakeProvider`) and loads anyway, like Dolphin
/// after its last refused request.
unsafe extern "C" fn vulkan_load(_: *const RetroGameInfo) -> bool {
    let mut callback = RetroHwRenderCallback {
        context_type: RETRO_HW_CONTEXT_VULKAN,
        context_reset: None,
        get_current_framebuffer: None,
        get_proc_address: None,
        depth: false,
        stencil: false,
        bottom_left_origin: true,
        version_major: 1,
        version_minor: 0,
        cache_context: true,
        context_destroy: None,
        debug_context: false,
    };
    // SAFETY: `struct retro_hw_render_callback *`.
    let accepted =
        unsafe { environment(RETRO_ENVIRONMENT_SET_HW_RENDER, (&raw mut callback).cast()) };
    assert!(!accepted, "Vulkan is refused");
    true
}

fn counting_core(load_game: unsafe extern "C" fn(*const RetroGameInfo) -> bool) -> CoreLibrary {
    STATE_CALLS.store(0, Ordering::SeqCst);
    CoreLibrary {
        load_game,
        serialize_size: counted_size,
        serialize: counted_serialize,
        unserialize: counted_unserialize,
        ..fake_core()
    }
}

/// Runs every state entry point; all must answer without reaching the core.
fn assert_no_state_call(session: &mut CoreSession) {
    assert!(!session.states_available());
    assert!(matches!(
        session.serialize(),
        Err(HostError::StatesNeedHwContext)
    ));
    assert!(matches!(
        session.unserialize(&[0; 4]),
        Err(HostError::StatesNeedHwContext)
    ));
    let mut buf = Vec::new();
    let ahead = SavestateContext::RunaheadSameInstance;
    assert!(session.serialize_into(&mut buf, ahead).is_err());
    assert!(session.unserialize_from(&[0; 4], ahead).is_err());
    assert_eq!(session.current_state_size(), 0);
    assert_eq!(STATE_CALLS.load(Ordering::SeqCst), 0, "no core state call");
}

#[test]
fn flagged_core_without_a_context_never_reaches_a_state_call() {
    let _serial = serial();
    let mut session = start_core(counting_core(software_load), &FakeProvider::default())
        .expect("loads in software");
    assert!(!session.uses_hw_context());
    assert!(session.states_available(), "unflagged: unchanged");
    session.require_hw_context_for_states(true);
    assert_no_state_call(&mut session);
    // Unflagged again: the same session reaches the core as before.
    session.require_hw_context_for_states(false);
    assert_eq!(session.current_state_size(), 4);
    session.serialize().expect("serialize");
    assert_eq!(
        STATE_CALLS.load(Ordering::SeqCst),
        3,
        "size, size, serialize"
    );
}

#[test]
fn refused_request_then_load_leaves_no_context_and_no_states() {
    let _serial = serial();
    let mut session = start_core(counting_core(vulkan_load), &FakeProvider::default())
        .expect("the core loads after the refusal");
    assert!(!session.uses_hw_context());
    assert!(session
        .refused_hw_render()
        .is_some_and(|r| r.contains("Vulkan")));
    session.require_hw_context_for_states(true);
    assert_no_state_call(&mut session);
}

#[test]
fn flagged_core_with_its_context_keeps_states() {
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("GL session");
    session.require_hw_context_for_states(true);
    assert!(session.states_available());
    session.serialize().expect("serialize with the context");
}

/// `start` with `SessionConfig::keep_library_loaded` = `keep` and a game `load_game`
/// accepts or fails.
fn start_kept(keep: bool, load_ok: bool) -> HostResult<CoreSession> {
    let rom = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let dir = std::path::Path::new(".");
    let provider = FakeProvider::default();
    let config = crate::session_config::SessionConfig {
        core_path: dir,
        rom_path: &rom,
        system_dir: dir,
        save_dir: dir,
        rom_data: None,
        staging_dir: None,
        core_options: &[],
        hw_render: Some(&provider),
        keep_library_loaded: keep,
    };
    let load_game = if load_ok { software_load } else { failed_load };
    let claim = crate::process_session_claim::ProcessSessionClaim::acquire()?;
    CoreSession::start(claim, counting_core(load_game), config)
}

/// Dolphin after "Could not boot": `FreeLibrary` would hang.
unsafe extern "C" fn failed_load(_: *const RetroGameInfo) -> bool {
    false
}

#[test]
fn kept_library_still_unloads_the_game_but_stays_mapped() {
    let _serial = serial();
    let session = start_kept(false, true).expect("session");
    assert!(session.core._lib.is_some(), "unflagged: freed at drop");
    drop(session);
    let session = start_kept(true, true).expect("session");
    assert!(session.core._lib.is_none(), "never freed");
    drop(session);
    let log = log_take();
    assert!(log.iter().any(|e| e == "unload") && log.iter().any(|e| e == "deinit"));
    // A failed load is kept mapped too (the library is never freed from the start).
    assert!(matches!(
        start_kept(true, false),
        Err(HostError::LoadGameRejected(_))
    ));
}

#[test]
fn abandoned_core_gets_no_unload_or_deinit() {
    let _serial = serial();
    let mut session =
        start_core(counting_core(software_load), &FakeProvider::default()).expect("session");
    session.abandon_core();
    assert!(session.core._lib.is_none());
    drop(session);
    let log = log_take();
    assert!(
        !log.iter().any(|e| e == "unload" || e == "deinit"),
        "{log:?}"
    );
}
