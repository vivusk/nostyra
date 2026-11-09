//! State-size rules on the fake core: cached by default, re-queried before every
//! serialize with `set_state_size_requery` (PPSSPP), exact sizes refused up front
//! (size 0 = no states), the size per savestate context (ProSystem) and the
//! framebuffer name answered to threads other than the session's (PPSSPP's emulation
//! thread).

use std::sync::atomic::Ordering;

use std::ffi::{c_int, c_void};

use super::fake_core_tests::{fake_core, start, start_core};
use super::tests::host_fns;
use crate::core_library_loader::CoreLibrary;
use crate::environment_callback::environment;
use crate::ffi_types::{RETRO_AV_ENABLE_FAST_SAVESTATES, RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE};
use crate::host_error::HostError;
use crate::hw_render_test_fakes::{log_take, serial, FakeProvider, FAKE_FBO, FAKE_STATE_SIZE};
use crate::savestate_context::SavestateContext::{Normal, RunaheadSameInstance as Fast};

#[test]
fn size_is_cached_unless_requery_is_on() {
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("session loads");
    assert!(!session.state_size_requery());
    assert_eq!(session.serialize().unwrap().len(), 4);
    FAKE_STATE_SIZE.store(8, Ordering::SeqCst);
    assert_eq!(session.serialize().unwrap().len(), 4, "cached");
    session.set_state_size_requery(true);
    assert!(session.state_size_requery());
    assert_eq!(session.serialize().unwrap().len(), 8, "re-queried");
    FAKE_STATE_SIZE.store(6, Ordering::SeqCst);
    assert_eq!(
        session.serialize().unwrap().len(),
        6,
        "every serialize asks again"
    );
}

#[test]
fn boot_size_is_never_used_for_a_later_save() {
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("session loads");
    session.set_state_size_requery(true);
    FAKE_STATE_SIZE.store(128, Ordering::SeqCst); // "before boot"
    assert_eq!(session.current_state_size(), 128);
    session.unserialize(&[0; 128]).expect("startup load");
    FAKE_STATE_SIZE.store(40, Ordering::SeqCst); // "booted"
    assert_eq!(session.current_state_size(), 40);
    assert_eq!(session.serialize().unwrap().len(), 40);
}

#[test]
fn exact_size_check_uses_the_fresh_size_when_requerying() {
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("session loads");
    session.set_exact_state_size(true);
    session.set_state_size_requery(true);
    session.unserialize(&[0; 4]).expect("current size");
    FAKE_STATE_SIZE.store(9, Ordering::SeqCst);
    assert!(session.unserialize(&[0; 4]).is_err(), "size changed since");
    session.unserialize(&[0; 9]).expect("new size");
}

#[test]
fn exact_state_size_refuses_other_lengths_without_calling_the_core() {
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("session loads");
    session.unserialize(&[0; 3]).expect("off: the core decides");
    session.set_exact_state_size(true);
    for wrong in [&[0u8; 3][..], &[0; 5]] {
        let refused = session.unserialize(wrong);
        assert!(matches!(refused, Err(HostError::UnserializeFailed)));
    }
    session.unserialize(&[0; 4]).expect("the core's size loads");
    drop(session);
    let calls = log_take().iter().filter(|e| *e == "unserialize").count();
    assert_eq!(calls, 2, "wrong sizes never reached the core");
}

#[test]
fn framebuffer_name_is_answered_off_the_session_thread() {
    let _serial = serial();
    let session = start(&FakeProvider::default()).expect("session loads");
    let (fb, _) = host_fns();
    // SAFETY: host callback; the fake context lives until `session` drops.
    let off_thread = std::thread::spawn(move || unsafe { fb() }).join().unwrap();
    assert_eq!(off_thread, FAKE_FBO);
    drop(session);
    // SAFETY: as above; no session (and no context) any more.
    let after = std::thread::spawn(move || unsafe { fb() }).join().unwrap();
    assert_eq!(after, 0, "withdrawn after the session's context_destroy");
}

#[test]
fn exact_size_zero_means_no_states_without_calling_the_core() {
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("session loads");
    session.set_exact_state_size(true);
    FAKE_STATE_SIZE.store(0, Ordering::SeqCst); // melonDS DS on a DSi title
    assert!(matches!(
        session.serialize(),
        Err(HostError::SerializeUnsupported)
    ));
    assert!(matches!(
        session.unserialize(&[0; 4]),
        Err(HostError::SerializeUnsupported)
    ));
    drop(session);
    assert!(!log_take()
        .iter()
        .any(|e| e.starts_with("serialize") || e == "unserialize"));
}

#[test]
fn reset_keeps_the_cached_size() {
    // melonDS DS: the size is the same before frame 1, over frames and after
    // `retro_reset` (M13 phase-00 spike), so a reset does not forget it.
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("session loads");
    assert_eq!(session.serialize().unwrap().len(), 4);
    session.reset();
    FAKE_STATE_SIZE.store(8, Ordering::SeqCst);
    assert_eq!(session.serialize().unwrap().len(), 4, "still cached");
}

/// ProSystem-like: 4 bytes Normal, 8 with `FAST_SAVESTATES`; refuses the other size.
fn context_size() -> usize {
    let mut flags: c_int = 0;
    // SAFETY: `int *` out-param of GET_AUDIO_VIDEO_ENABLE, answered by the host.
    unsafe {
        environment(
            RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE,
            (&raw mut flags).cast(),
        )
    };
    if flags & RETRO_AV_ENABLE_FAST_SAVESTATES != 0 {
        8
    } else {
        4
    }
}
unsafe extern "C" fn sized_by_context() -> usize {
    context_size()
}
unsafe extern "C" fn serialize_checked(_: *mut c_void, size: usize) -> bool {
    size == context_size()
}
unsafe extern "C" fn unserialize_checked(_: *const c_void, size: usize) -> bool {
    size == context_size()
}

#[test]
fn each_savestate_context_gets_its_own_size() {
    let _serial = serial();
    let core = CoreLibrary {
        serialize_size: sized_by_context,
        serialize: serialize_checked,
        unserialize: unserialize_checked,
        ..fake_core()
    };
    let mut session = start_core(core, &FakeProvider::default()).expect("session loads");
    session.set_exact_state_size(true);
    let mut buf = Vec::new();
    assert_eq!(session.serialize_into(&mut buf, Fast).unwrap(), 8);
    session
        .unserialize_from(&buf, Fast)
        .expect("fast state in fast context");
    assert!(
        session.unserialize_from(&buf, Normal).is_err(),
        "exact size per context"
    );
    assert_eq!(session.serialize_into(&mut buf, Normal).unwrap(), 4);
    session
        .unserialize_from(&buf, Normal)
        .expect("normal state");
    assert_eq!(
        session.serialize_into(&mut buf, Fast).unwrap(),
        8,
        "both cached"
    );
}
