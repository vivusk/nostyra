//! Loading real cores in in-binary tests: registry core paths, software sessions
//! (`load_session*`), OpenGL sessions (`load_session_hw`), sessions with a shared
//! system folder (`load_session_in`) and the GL test gate.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};

use libretro_host::{CoreSession, HwRenderProvider, SessionConfig};

use crate::runner_real_core_suite::repo;

/// `cores/<library stem>.<dll ext>` of a registry core (fetched by its script).
pub(crate) fn core_path(core_id: &str) -> PathBuf {
    let core = system_catalog::core(core_id).expect("registry core");
    repo(&format!(
        "cores/{}.{}",
        core.library_stem,
        std::env::consts::DLL_EXTENSION
    ))
}

/// Loads `core_id` with `rom`; saves go to `dir`.
pub(crate) fn load_session(core_id: &str, rom: &Path, dir: &Path) -> CoreSession {
    load_session_with(core_id, rom, dir, &[])
}

/// [`load_session`] answering `core_options` (`GET_VARIABLE`).
pub(crate) fn load_session_with(
    core_id: &str,
    rom: &Path,
    dir: &Path,
    core_options: &[(String, String)],
) -> CoreSession {
    load_session_in(core_id, rom, (dir, dir), core_options, None)
}

/// [`load_session_with`] for cores that render on the GPU: `provider` builds their
/// graphics context and must outlive the session.
pub(crate) fn load_session_hw(
    core_id: &str,
    rom: &Path,
    dir: &Path,
    core_options: &[(String, String)],
    provider: &dyn HwRenderProvider,
) -> CoreSession {
    load_session_in(core_id, rom, (dir, dir), core_options, Some(provider))
}

/// Loads `core_id` with `rom`, `(system, saves)` folders apart: suites share one
/// system folder holding the core's system files (PPSSPP) while saves stay per test.
pub(crate) fn load_session_in(
    core_id: &str,
    rom: &Path,
    (system_dir, save_dir): (&Path, &Path),
    core_options: &[(String, String)],
    hw_render: Option<&dyn HwRenderProvider>,
) -> CoreSession {
    let core = core_path(core_id);
    assert!(
        core.exists(),
        "missing {}; run scripts/fetch-{core_id}-core first",
        core.display()
    );
    let spec = system_catalog::core(core_id).unwrap();
    let mut session = CoreSession::load(SessionConfig {
        core_path: &core,
        rom_path: rom,
        system_dir,
        save_dir,
        rom_data: None,
        staging_dir: None,
        core_options,
        hw_render,
        keep_library_loaded: spec.keep_library_loaded, // as the runner
    })
    .expect("load core + ROM");
    session.require_hw_context_for_states(spec.hw_render_required);
    session.set_exact_state_size(spec.exact_state_size); // as the runner
    session.set_state_size_requery(spec.requery_state_size);
    session.set_controller_port_devices(spec.port_devices);
    session
}

/// True when OpenGL tests can run here. Without an OpenGL 3.3 context (CI without a
/// GPU, macOS test threads) they skip with a note, unless `NOSTYRA_REQUIRE_GL=1`
/// turns the skip into a failure. `NOSTYRA_FORCE_NO_GL=1` (the runner's debug hook)
/// skips them too, as on a machine without OpenGL. Call it under `CORE_LOCK` (one
/// session at a time).
pub(crate) fn gl_test_gate() -> bool {
    let required = std::env::var("NOSTYRA_REQUIRE_GL").is_ok_and(|v| v == "1");
    if std::env::var("NOSTYRA_FORCE_NO_GL").is_ok_and(|v| v == "1") {
        assert!(!required, "NOSTYRA_REQUIRE_GL=1 with NOSTYRA_FORCE_NO_GL=1");
        eprintln!("skipped: NOSTYRA_FORCE_NO_GL=1");
        return false;
    }
    if cfg!(target_os = "macos") {
        assert!(!required, "GL tests cannot run on macOS test threads");
        eprintln!("skipped: SDL windows need the main thread on macOS");
        return false;
    }
    match hw_render_gl::probe_gl_context() {
        Ok(()) => true,
        Err(error) if required => panic!("NOSTYRA_REQUIRE_GL=1 but no OpenGL 3.3: {error}"),
        Err(error) => {
            eprintln!("skipped: no OpenGL 3.3 context ({error})");
            false
        }
    }
}
