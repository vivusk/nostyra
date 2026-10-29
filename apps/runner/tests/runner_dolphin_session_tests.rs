//! GameCube on Dolphin through the runner binary, without OpenGL: a refused graphics
//! request is fatal (exit 6, generic reason, no unload of the black Null renderer),
//! and a session on the Null renderer (test `--core-option`) refuses every state
//! action before the core (its state calls segfault, M14 spike) and writes nothing
//! outside the game's save folder; an unbootable file ends with exit 4 instead of
//! hanging in `FreeLibrary` (library kept loaded from the load on). Skipped without
//! the core (`scripts/fetch-dolphin-core.*`). A 32-byte in-code `.dol` is the game.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use disc_image::test_nintendo_images::dol;
use ipc_protocol::StateSidecar;
use support::{game_saves_dir, output_within, registry_core, result_line, runner, scratch};
use system_catalog::GC;

const FETCH: &str = "fetch-dolphin-core";
/// `b .` (the core boots it; nothing has to happen on screen).
const BRANCH_TO_SELF: u32 = 0x4800_0000;

fn dolphin() -> Option<PathBuf> {
    let core = system_catalog::core_for_system(GC.id).unwrap();
    registry_core(core.id, FETCH)
}

fn gc_dol(data: &Path) -> PathBuf {
    let rom = data.join("loop.dol");
    // Dolphin refuses to boot a text section that is not 32-byte aligned.
    std::fs::write(&rom, dol(&[BRANCH_TO_SELF; 8], &[])).unwrap();
    rom
}

/// Exit 4 at once: a hard exit (no DLL detach, which blocked `ExitProcess` about once
/// in 15 runs after a failed boot); the deadline fails the test instead of hanging.
#[test]
fn unbootable_file_exits_4_without_hanging() {
    let Some(core) = dolphin() else { return };
    let data = scratch("dolphin-unbootable");
    let rom = data.join("short.dol");
    std::fs::write(&rom, dol(&[BRANCH_TO_SELF; 2], &[])).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"));
    command.arg("--core").arg(&core).arg("--rom").arg(&rom);
    command.arg("--data-dir").arg(&data);
    command.args([
        "--core-option",
        "dolphin_renderer=Null",
        "--headless-frames",
        "5",
    ]);
    let out = output_within(command, Duration::from_secs(60), "unbootable DOL run");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_ROM_LOAD),
        "{stderr}"
    );
}

#[test]
fn refused_graphics_request_exits_6_with_a_generic_reason() {
    if !cfg!(debug_assertions) {
        eprintln!("skipped: NOSTYRA_FORCE_NO_GL is read by debug runner builds only");
        return;
    }
    let Some(core) = dolphin() else { return };
    let data = scratch("dolphin-refused");
    let rom = gc_dol(&data);
    let out = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .env("NOSTYRA_FORCE_NO_GL", "refuse")
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
    let reason = stderr
        .lines()
        .rfind(|line| line.contains("graphics:"))
        .expect("graphics error line");
    assert!(reason.contains("OpenGL 3.3"), "{reason}");
    assert!(
        !reason.contains("Vulkan"),
        "never the last refused kind: {reason}"
    );
}

#[test]
fn null_renderer_session_refuses_states_without_a_core_call() {
    let Some(core) = dolphin() else { return };
    let data = scratch("dolphin-null");
    let rom = gc_dol(&data);
    let saves = game_saves_dir(&data, GC.id, &rom);
    // A slot that would be loaded if states were available (never reaches the core).
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("slot2.state"), [0u8; 64]).unwrap();
    let sidecar = StateSidecar {
        core_name: "x".into(),
        ..StateSidecar::default()
    };
    std::fs::write(
        saves.join("slot2.json"),
        serde_json::to_vec(&sidecar).unwrap(),
    )
    .unwrap();
    let null = ["--core-option", "dolphin_renderer=Null"];
    let args = [&null[..], &["--headless-frames", "30", "--load-state", "2"]].concat();
    let args = [&args[..], &["--headless-save-slot", "1"]].concat();
    let out = runner(&core, &rom, &data, &args);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "no crash: {stderr}");
    assert!(result_line(&out).ends_with("loaded=false"), "{stderr}");
    assert!(
        stderr.contains("save states need hardware rendering"),
        "{stderr}"
    );
    assert!(
        stderr.contains("unavailable without hardware rendering"),
        "{stderr}"
    );
    assert!(!saves.join("slot1.state").exists(), "no state written");
    assert!(!data.join("cheats").exists(), "no <data>/cheats litter");
}
