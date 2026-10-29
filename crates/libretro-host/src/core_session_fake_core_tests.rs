//! The real `CoreSession` driven against a fake core (extern "C" stubs backed by this
//! process): proves the `load`/`run`/`reset`/`serialize`/`Drop` order of the
//! hardware-render lifecycle, including the failed-create path.

use std::ffi::{c_uint, c_void};
use std::path::Path;

use super::tests::{fake_run, gl33_request, host_fns, HOST_FNS};
use super::*;
use crate::core_library_loader::CoreLibrary;
use crate::environment_callback::environment;
use crate::ffi_hw_render_types::RETRO_ENVIRONMENT_SET_HW_RENDER;
use crate::ffi_types::*;
use crate::frame_sink_traits::{NullAudioSink, NullInputSource, NullVideoSink};
use crate::hw_render_test_fakes::{log, log_take, serial, FakeProvider, FAKE_STATE_SIZE};
use crate::process_session_claim::ProcessSessionClaim;
use crate::session_config::SessionConfig;

unsafe extern "C" fn api_version() -> c_uint {
    1
}
unsafe extern "C" fn system_info(_: *mut RetroSystemInfo) {}
unsafe extern "C" fn av_info(out: *mut RetroSystemAvInfo) {
    // SAFETY: valid out-param supplied by the session.
    unsafe {
        (*out).geometry.max_width = 640;
        (*out).geometry.max_height = 480;
    }
}
unsafe extern "C" fn set_env(_: RetroEnvironmentFn) {}
unsafe extern "C" fn set_video(_: RetroVideoRefreshFn) {}
unsafe extern "C" fn set_audio(_: RetroAudioSampleFn) {}
unsafe extern "C" fn set_batch(_: RetroAudioSampleBatchFn) {}
unsafe extern "C" fn set_poll(_: RetroInputPollFn) {}
unsafe extern "C" fn set_state(_: RetroInputStateFn) {}
unsafe extern "C" fn init() {}
unsafe extern "C" fn deinit() {
    log("deinit");
}
unsafe extern "C" fn core_reset() {
    log("core_reset");
}
unsafe extern "C" fn run() {
    fake_run();
}
unsafe extern "C" fn state_size() -> usize {
    FAKE_STATE_SIZE.load(std::sync::atomic::Ordering::SeqCst)
}
unsafe extern "C" fn serialize(_: *mut c_void, _: usize) -> bool {
    let (fb, _) = host_fns();
    // SAFETY: host callback; re-entered from inside `retro_serialize` as Mupen does.
    log(format!("serialize fb={}", unsafe { fb() }));
    true
}
unsafe extern "C" fn unserialize(_: *const c_void, _: usize) -> bool {
    log("unserialize");
    true
}
unsafe extern "C" fn load_game(_: *const RetroGameInfo) -> bool {
    let mut callback = gl33_request();
    // SAFETY: `struct retro_hw_render_callback *`.
    assert!(unsafe { environment(RETRO_ENVIRONMENT_SET_HW_RENDER, (&raw mut callback).cast()) });
    *HOST_FNS.lock().unwrap_or_else(|e| e.into_inner()) = Some((
        callback.get_current_framebuffer.expect("filled"),
        callback.get_proc_address.expect("filled"),
    ));
    true
}
unsafe extern "C" fn unload_game() {
    log("unload");
}
unsafe extern "C" fn mem_data(_: c_uint) -> *mut c_void {
    std::ptr::null_mut()
}
unsafe extern "C" fn mem_size(_: c_uint) -> usize {
    0
}

unsafe extern "C" fn port_device(port: c_uint, device: c_uint) {
    log(format!("port {port}={device}"));
}

pub(crate) fn fake_core() -> CoreLibrary {
    CoreLibrary {
        api_version,
        get_system_info: system_info,
        get_system_av_info: av_info,
        set_environment: set_env,
        set_video_refresh: set_video,
        set_audio_sample: set_audio,
        set_audio_sample_batch: set_batch,
        set_input_poll: set_poll,
        set_input_state: set_state,
        init,
        deinit,
        reset: core_reset,
        run,
        serialize_size: state_size,
        serialize,
        unserialize,
        load_game,
        unload_game,
        get_memory_data: mem_data,
        get_memory_size: mem_size,
        set_controller_port_device: Some(port_device),
        _lib: Some(CoreLibrary::this_process()),
    }
}

pub(crate) fn start(provider: &FakeProvider) -> HostResult<CoreSession> {
    start_core(fake_core(), provider)
}

/// `start` with a changed fake core (`CoreLibrary { load_game: …, ..fake_core() }`).
pub(crate) fn start_core(core: CoreLibrary, provider: &FakeProvider) -> HostResult<CoreSession> {
    let rom = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let dir = Path::new(".");
    let config = SessionConfig {
        core_path: dir,
        rom_path: &rom,
        system_dir: dir,
        save_dir: dir,
        rom_data: None,
        staging_dir: None,
        core_options: &[],
        hw_render: Some(provider),
        keep_library_loaded: false,
    };
    CoreSession::start(ProcessSessionClaim::acquire()?, core, config)
}

fn position(log: &[String], entry: &str) -> usize {
    log.iter()
        .position(|e| e.starts_with(entry))
        .unwrap_or_else(|| panic!("{entry} missing from {log:?}"))
}

#[test]
fn session_follows_the_hw_lifecycle_order_end_to_end() {
    let _serial = serial();
    let mut session = start(&FakeProvider::default()).expect("session loads");
    assert!(
        session.ctx().hw_provider.is_none(),
        "provider cleared after load"
    );
    let mut video = NullVideoSink::default();
    let mut audio = NullAudioSink::default();
    session
        .run_frame(&mut video, &mut audio, &mut NullInputSource)
        .expect("frame runs");
    assert_eq!(video.frames, 1);
    session.serialize().expect("serialize");
    session.unserialize(&[0; 4]).expect("unserialize");
    session.reset();
    drop(session);
    let log = log_take();
    let order = ["create", "reset fb", "run fb", "deliver", "serialize fb=7"];
    for pair in order.windows(2) {
        assert!(position(&log, pair[0]) < position(&log, pair[1]), "{log:?}");
    }
    assert!(position(&log, "serialize") < position(&log, "unserialize"));
    assert!(position(&log, "unserialize") < position(&log, "core_reset"));
    assert!(position(&log, "core_reset") < position(&log, "destroy"));
    assert!(position(&log, "destroy") < position(&log, "unload"));
    assert!(position(&log, "unload") < position(&log, "deinit"));
    assert_eq!(log.last().map(String::as_str), Some("drop context"));
    // Make-current precedes each core call that may render.
    for call in [
        "run fb",
        "serialize fb",
        "unserialize",
        "core_reset",
        "destroy",
    ] {
        let at = position(&log, call);
        assert_eq!(log[at - 1], "current", "{call}: {log:?}");
    }
}

#[test]
fn failed_create_unwinds_without_destroy_unload_or_deinit() {
    let _serial = serial();
    let result = start(&FakeProvider { fail_create: true });
    assert!(matches!(result, Err(HostError::HwRenderUnavailable(_))));
    // GLideN64 crashes in `retro_deinit`, PPSSPP in `retro_unload_game` (`hw::destroy`).
    assert_eq!(log_take(), ["create 640x480"]);
}
