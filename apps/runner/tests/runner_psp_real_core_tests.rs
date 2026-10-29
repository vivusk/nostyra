//! Sony PSP on PPSSPP through the runner binary: missing system files (exit 3), a
//! PS1 image forced to PSP (exit 4), a core shutdown on a bad executable (exit 5,
//! the core's last error line as the reason), the "no OpenGL" exit (6, no crash at
//! process exit), and a headless smoke run on the test UMD that leaves the game's
//! savedata on the memory stick (= its save folder). Software backend
//! (`ppsspp_backend=none`): no GL needed. Skipped (with a note) without the core
//! (`scripts/fetch-ppsspp-core.*`).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use disc_image::test_images::{ps1_sectors, to_iso, US_LICENCE};
use disc_image::test_psp_images::psp_iso_sectors;
use support::{game_saves_dir, registry_core, repo, result_line, runner, scratch};
use system_catalog::PSP;

const CORE: &str = "ppsspp";
const FETCH: &str = "fetch-ppsspp-core";
const SOFTWARE: &str = "ppsspp_backend=none";

fn ppsspp() -> Option<PathBuf> {
    let core = system_catalog::core_for_system(PSP.id).unwrap();
    assert_eq!(core.id, CORE);
    registry_core(CORE, FETCH)
}

/// Headless frames are not paced and PPSSPP boots on its own thread (~1 s in a debug
/// runner): enough frames to reach the boot, ended early by a shutdown or an error.
fn run(core: &Path, rom: &Path, data: &Path) -> (Output, String) {
    run_frames(core, rom, data, "100000")
}

fn run_frames(core: &Path, rom: &Path, data: &Path, frames: &str) -> (Output, String) {
    let extra = ["--system", PSP.id, "--headless-frames", frames];
    let out = runner(
        core,
        rom,
        data,
        &[&extra[..], &["--core-option", SOFTWARE]].concat(),
    );
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (out, stderr)
}

/// The test UMD: valid `PSP_GAME` tree, `EBOOT.BIN` = 4 junk bytes.
fn junk_eboot_iso(dir: &Path) -> PathBuf {
    let iso = dir.join("junk-eboot.iso");
    std::fs::write(&iso, to_iso(&psp_iso_sectors())).unwrap();
    iso
}

#[test]
fn missing_system_files_are_exit_3_with_the_redownload_hint() {
    let Some(core) = ppsspp() else { return };
    let data = scratch("psp-no-assets");
    // A core folder without `PPSSPP/` beside the library, an empty system folder.
    let lonely = data.join("cores");
    std::fs::create_dir_all(&lonely).unwrap();
    let copy = lonely.join(core.file_name().unwrap());
    std::fs::copy(&core, &copy).unwrap();
    let (out, stderr) = run(&copy, &junk_eboot_iso(&data), &data);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_CORE_LOAD),
        "{stderr}"
    );
    assert!(
        stderr.contains("PPSSPP system files are missing ("),
        "{stderr}"
    );
    assert!(
        stderr.contains("Re-download the core in Settings → Cores."),
        "{stderr}"
    );
}

#[test]
fn ps1_image_forced_to_psp_is_a_rom_error() {
    let Some(core) = ppsspp() else { return };
    let data = scratch("psp-ps1");
    let iso = data.join("ps1.iso");
    let sectors = ps1_sectors(Some(US_LICENCE), r"BOOT = cdrom:\SLUS_012.34;1");
    std::fs::write(&iso, to_iso(&sectors)).unwrap();
    let (out, stderr) = run(&core, &iso, &data);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_ROM_LOAD),
        "{stderr}"
    );
    assert!(stderr.contains("not a PSP UMD image"), "{stderr}");
}

/// PPSSPP cannot load the 4-byte `EBOOT.BIN`, logs why at error level and requests
/// a shutdown without `SET_MESSAGE`: exit 5 with that log line as the reason. The
/// system files were installed from beside the core first.
#[test]
fn bad_executable_ends_the_run_with_the_core_reason() {
    let Some(core) = ppsspp() else { return };
    let data = scratch("psp-shutdown");
    let (out, stderr) = run(&core, &junk_eboot_iso(&data), &data);
    assert_eq!(
        out.status.code(),
        Some(ipc_protocol::EXIT_RUNTIME),
        "{stderr}"
    );
    assert!(
        stderr.contains("the core stopped after an emulation error: "),
        "{stderr}"
    );
    assert!(
        stderr.contains("error: [Boot] Failed to load executable"),
        "{stderr}"
    );
    assert!(
        data.join("system/PPSSPP/compat.ini").is_file(),
        "assets installed"
    );
}

/// `NOSTYRA_FORCE_NO_GL=1` (debug runner only) fails the context creation: exit 6,
/// not the crash PPSSPP's destructors cause at a normal exit.
#[test]
fn forced_gl_failure_exits_6_without_a_crash() {
    if !cfg!(debug_assertions) {
        eprintln!("skipped: NOSTYRA_FORCE_NO_GL is read by debug runner builds only");
        return;
    }
    let Some(core) = ppsspp() else { return };
    let data = scratch("psp-no-gl");
    let out = Command::new(env!("CARGO_BIN_EXE_nostyra-runner"))
        .env("NOSTYRA_FORCE_NO_GL", "1")
        .args(["--core".as_ref(), core.as_os_str()])
        .args(["--rom".as_ref(), junk_eboot_iso(&data).as_os_str()])
        .args(["--data-dir".as_ref(), data.as_os_str()])
        .args(["--system", PSP.id, "--headless-frames", "5"])
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

/// The test UMD (`test-assets/roms/src/build-nostyra-test-psp.py`) boots, renders and
/// its boot-time AUTOSAVE lands in `saves/psp/<game> [ID]/PSP/SAVEDATA/`.
#[test]
fn test_umd_headless_smoke_writes_savedata_to_the_game_folder() {
    let Some(core) = ppsspp() else { return };
    let iso = repo("test-assets/roms/nostyra-test-psp.iso");
    let data = scratch("psp-smoke");
    let (out, stderr) = run_frames(&core, &iso, &data, "6000");
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    let line = result_line(&out);
    assert!(line.contains(" nonblack=true "), "{line}");
    let save = game_saves_dir(&data, PSP.id, &iso).join("PSP/SAVEDATA/NSTY00001DATA00");
    assert!(save.join("DATA.BIN").is_file(), "{}", save.display());
}
