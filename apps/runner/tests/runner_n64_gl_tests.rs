//! Nintendo 64 on Mupen64Plus-Next through the runner binary: the "no OpenGL" exit
//! (6) and a GL headless smoke run. Skipped (with a note) without the core
//! (`scripts/fetch-mupen64plus-next-core.*`); the smoke run also needs the N64 test
//! ROM (`test-assets/roms/nostyra-test.z64`) and OpenGL (`gl_test_gate`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::PathBuf;
use std::process::Command;

use support::{gl_test_gate, registry_core, repo, result_line, runner, scratch};

const CORE: &str = "mupen64plus-next";
const FETCH: &str = "fetch-mupen64plus-next-core";

fn n64_core() -> Option<PathBuf> {
    let core = system_catalog::core_for_system(system_catalog::NINTENDO_64.id).unwrap();
    assert_eq!(core.id, CORE);
    registry_core(CORE, FETCH)
}

/// `NOSTYRA_FORCE_NO_GL=1` (debug runner only) fails the context creation after
/// `retro_load_game`: exit 6 with the graphics reason, no crash in the core's
/// teardown. A header-only z64 is enough: the context is made right after the load.
#[test]
fn forced_gl_failure_exits_with_the_graphics_code() {
    if !cfg!(debug_assertions) {
        eprintln!("skipped: NOSTYRA_FORCE_NO_GL is read by debug runner builds only");
        return;
    }
    let Some(core) = n64_core() else { return };
    let data = scratch("n64-no-gl");
    let rom = data.join("header-only.z64");
    let mut bytes = vec![0u8; 8 * 1024];
    bytes[..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
    std::fs::write(&rom, bytes).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .env("NOSTYRA_FORCE_NO_GL", "1")
        .arg("--core")
        .arg(&core)
        .arg("--rom")
        .arg(&rom)
        .arg("--data-dir")
        .arg(&data)
        .args(["--headless-frames", "5"])
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

/// The test ROM renders through the session's OpenGL context and the readback
/// reaches the headless frame buffer.
#[test]
fn gl_headless_smoke_is_not_black() {
    let Some(core) = n64_core() else { return };
    let rom = repo("test-assets/roms/nostyra-test.z64");
    if !rom.exists() {
        eprintln!("skipped: {} not built yet", rom.display());
        return;
    }
    if !gl_test_gate() {
        return;
    }
    let data = scratch("n64-gl-smoke");
    let system = system_catalog::NINTENDO_64.id;
    let args = ["--system", system, "--headless-frames", "120"];
    let out = runner(&core, &rom, &data, &args);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    let line = result_line(&out);
    assert!(
        line.starts_with("frames=120 ") && line.contains(" nonblack=true "),
        "{line}"
    );
}
