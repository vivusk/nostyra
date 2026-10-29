//! Sega Dreamcast on Flycast through the runner binary: broken discs (exit 4), the
//! "no OpenGL" exit (6), a core shutdown request ending the run with the core's
//! reason (exit 5), and a headless smoke run on the test GDI (phase 07). Skipped
//! (with a note) without the core (`scripts/fetch-flycast-core.*`); runs that boot
//! the core also need OpenGL (`gl_test_gate`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use disc_image::test_gd_images::write_gdi;
use support::{game_saves_dir, gl_test_gate, registry_core, repo, result_line, runner, scratch};
use system_catalog::DREAMCAST;

const CORE: &str = "flycast";
/// The GL probe initialises SDL in this test process: one GL test at a time.
static GL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn gl_gate() -> Option<std::sync::MutexGuard<'static, ()>> {
    let guard = GL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    gl_test_gate().then_some(guard)
}
const FETCH: &str = "fetch-flycast-core";

fn flycast() -> Option<PathBuf> {
    let core = system_catalog::core_for_system(DREAMCAST.id).unwrap();
    assert_eq!(core.id, CORE);
    registry_core(CORE, FETCH)
}

fn run(core: &Path, rom: &Path, data: &Path) -> (Output, String) {
    let out = runner(
        core,
        rom,
        data,
        &["--system", DREAMCAST.id, "--headless-frames", "30"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (out, stderr)
}

#[test]
fn broken_discs_are_rom_errors_with_the_reason() {
    let Some(core) = flycast() else { return };
    let data = scratch("dc-broken");
    let junk = data.join("junk.gdi");
    std::fs::write(&junk, "this is not a gdi sheet\n").unwrap();
    let (out, stderr) = run(&core, &junk, &data);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_ROM_LOAD),
        "{stderr}"
    );
    assert!(stderr.contains("not a valid GDI sheet"), "{stderr}");
    let gdi = write_gdi(&data, "game", "NSTY-00001", false);
    std::fs::remove_file(data.join("game-track03.bin")).unwrap();
    let (out, stderr) = run(&core, &gdi, &data);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_ROM_LOAD),
        "{stderr}"
    );
    assert!(
        stderr.contains("track file 'game-track03.bin' not found"),
        "{stderr}"
    );
}

/// `NOSTYRA_FORCE_NO_GL=1` (debug runner only) fails the context creation after
/// `retro_load_game`: exit 6 with the graphics reason.
#[test]
fn forced_gl_failure_exits_with_the_graphics_code() {
    if !cfg!(debug_assertions) {
        eprintln!("skipped: NOSTYRA_FORCE_NO_GL is read by debug runner builds only");
        return;
    }
    let Some(core) = flycast() else { return };
    let data = scratch("dc-no-gl");
    let gdi = write_gdi(&data, "game", "NSTY-00001", false);
    let out = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .env("NOSTYRA_FORCE_NO_GL", "1")
        .args(["--core".as_ref(), core.as_os_str()])
        .args(["--rom".as_ref(), gdi.as_os_str()])
        .args(["--data-dir".as_ref(), data.as_os_str()])
        .args(["--system", DREAMCAST.id, "--headless-frames", "5"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_GRAPHICS),
        "{stderr}"
    );
    assert!(stderr.contains("forced by NOSTYRA_FORCE_NO_GL"), "{stderr}");
}

/// The fixture GDI has IP.BIN but no file system: Flycast's HLE BIOS cannot find
/// the boot file, says so and requests a shutdown. The run ends with that reason
/// instead of emulating a frozen screen.
#[test]
fn core_shutdown_request_ends_the_run_with_its_reason() {
    let Some(core) = flycast() else { return };
    let Some(_gl) = gl_gate() else { return };
    let data = scratch("dc-shutdown");
    let gdi = write_gdi(&data, "game", "NSTY-00001", false);
    let (out, stderr) = run(&core, &gdi, &data);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_RUNTIME),
        "{stderr}"
    );
    assert!(
        stderr.contains("the core stopped after an emulation error"),
        "{stderr}"
    );
    assert!(stderr.contains("bootfile"), "{stderr}");
    assert!(
        stderr.contains("Sega Dreamcast BIOS: HLE BIOS (built-in)"),
        "{stderr}"
    );
}

/// The test GDI (built by `test-assets/roms/src/build-nostyra-test-dreamcast.py`)
/// boots under the HLE BIOS, renders through OpenGL, and the core creates the first
/// pad's VMU (`<product>.A1.bin`, 128 KiB) in the game's save folder.
#[test]
fn test_gdi_headless_smoke_renders_and_creates_the_vmu() {
    let Some(core) = flycast() else { return };
    let gdi = repo("test-assets/roms/nostyra-test-dc.gdi");
    let Some(_gl) = gl_gate() else { return };
    let data = scratch("dc-smoke");
    let (out, stderr) = run(&core, &gdi, &data);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    let line = result_line(&out);
    assert!(
        line.starts_with("frames=30 ") && line.contains(" nonblack=true "),
        "{line}"
    );
    let vmu = game_saves_dir(&data, DREAMCAST.id, &gdi).join("NSTY-00001.A1.bin");
    assert_eq!(
        std::fs::metadata(&vmu).map(|m| m.len()).ok(),
        Some(128 * 1024),
        "{}",
        vmu.display()
    );
    assert!(
        data.join("system/dc/dc_nvmem.bin").is_file(),
        "flash in the shared system folder"
    );
}
