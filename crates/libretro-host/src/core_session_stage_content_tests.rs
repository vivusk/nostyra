//! `SessionConfig::stage_content` on a memory-loading fake core (Handy-like): the
//! preloaded bytes are staged as `content.<ext>`, the core gets that path **and** the
//! bytes; without the flag nothing is staged.

use std::ffi::CStr;
use std::path::Path;
use std::sync::Mutex;

use crate::core_library_loader::CoreLibrary;
use crate::core_session::CoreSession;
use crate::core_session_hw_render::fake_core_tests::fake_core;
use crate::ffi_types::RetroGameInfo;
use crate::hw_render_test_fakes::serial;
use crate::process_session_claim::ProcessSessionClaim;
use crate::session_config::SessionConfig;

/// (path the core got, staged file bytes, data bytes) of a load.
type Seen = (String, Vec<u8>, Vec<u8>);

/// What the last load saw.
static SEEN: Mutex<Option<Seen>> = Mutex::new(None);

unsafe extern "C" fn recording_load(game: *const RetroGameInfo) -> bool {
    // SAFETY: the host passes a valid game info with a C path and `size` data bytes.
    let (path, data) = unsafe {
        let game = &*game;
        let path = CStr::from_ptr(game.path).to_string_lossy().into_owned();
        let data = std::slice::from_raw_parts(game.data.cast::<u8>(), game.size).to_vec();
        (path, data)
    };
    let file = std::fs::read(&path).unwrap_or_default();
    *SEEN.lock().unwrap_or_else(|e| e.into_inner()) = Some((path, file, data));
    true
}

fn load(stage_content: bool, staging: &Path) -> Seen {
    let rom = staging.join("Original Game.lnx");
    std::fs::write(&rom, [1u8, 2, 3]).unwrap();
    let core = CoreLibrary {
        load_game: recording_load,
        ..fake_core()
    };
    let config = SessionConfig {
        core_path: staging,
        rom_path: &rom,
        system_dir: staging,
        save_dir: staging,
        rom_data: Some(&[9, 9]), // "patched"
        staging_dir: Some(staging),
        core_options: &[],
        hw_render: None,
        keep_library_loaded: false,
        stage_content,
    };
    let session = CoreSession::start(ProcessSessionClaim::acquire().unwrap(), core, config);
    let seen = SEEN.lock().unwrap_or_else(|e| e.into_inner()).take();
    drop(session);
    seen.expect("load_game called")
}

#[test]
fn stage_content_core_gets_a_staged_path_and_the_bytes() {
    let _serial = serial();
    let dir = tempfile::Builder::new()
        .prefix("nostyra-stage-")
        .tempdir()
        .unwrap();
    let (path, file, data) = load(true, dir.path());
    assert_eq!(Path::new(&path).file_name().unwrap(), "content.lnx");
    assert_eq!(
        (file, data),
        (vec![9, 9], vec![9, 9]),
        "patched bytes both ways"
    );
    let (path, file, data) = load(false, dir.path());
    assert!(path.ends_with("Original Game.lnx"), "{path}");
    assert_eq!(
        (file, data),
        (vec![1, 2, 3], vec![9, 9]),
        "no staging without the flag"
    );
}
