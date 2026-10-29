//! GameCube and Wii on Dolphin (`scripts/fetch-dolphin-core.*`) with the self-written
//! test DOLs (`test-assets/roms/src/build-nostyra-test-gc.py`). Registered as a child
//! of `psp_real_core_suite_tests`: the suites share the runner's test binary with the
//! other cores (M14 spike step 9: in-process coexistence is fine while Dolphin stays
//! loaded, which the loader takes from the registry exactly as the runner does).
//!
//! - Null renderer (`dolphin_renderer=Null`, every OS): boot, MEM1 field counter, pad
//!   words, audio, reset, Wii mode, and the clean state refusal (a state call under
//!   Null segfaults the process, so none may reach the core).
//! - OpenGL (`gl_test_gate`: dev GPU, Linux llvmpipe in CI; not Windows CI): pixels,
//!   then every state suite (slot round trip through the guard, damaged / other
//!   version / other disc refusals, startup resume, keep-loaded thread exit).
//!
//! The program mirrors its state into MEM1 (`RETRO_MEMORY_SYSTEM_RAM`); the offsets
//! below must match `test-assets/roms/src/gc_test_program.py`. The GC DOL reports
//! 50 fps (PAL region) while its VI runs NTSC: nothing here asserts fps or pacing.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use libretro_host::{CoreSession, HwRenderProvider};
use system_catalog::{CoreSpec, SystemSpec, GC};

use crate::core_system_assets::{cores_dir_of, ensure_core_assets};
use crate::real_core_session_loader::load_session_in;
use crate::runner_core_options;
use crate::runner_real_core_suite::{core_path, repo, scratch};

#[path = "checks_tests.rs"]
mod checks;
#[path = "gl_state_suite_tests.rs"]
mod gl_state_suite_tests;
#[path = "gl_suite_tests.rs"]
mod gl_suite_tests;
#[path = "null_suite_tests.rs"]
mod null_suite_tests;

// The DOL paths live in a `_tests.rs` file (literal gate: no extensions in mod.rs).
pub(crate) use checks::{GC_DOL, WII_DOL};

/// MEM1 debug block (physical address = `SYSTEM_RAM` offset) and its word offsets.
pub(crate) const DEBUG_BLOCK: usize = 0x0020_0000;
pub(crate) const MAGIC: u32 = 0x4E53_5459; // +0 "NSTY"
pub(crate) const FIELDS: usize = 4; // VI field counter (one per retro_run)
pub(crate) const SI_IN_HI: usize = 8; // SI channel 0: buttons, stick X, stick Y
pub(crate) const SI_IN_LO: usize = 12; // C-stick X, C-stick Y, analog L, analog R
pub(crate) const HID4_AT_BOOT: usize = 28; // HID4 as Dolphin set it (Wii mode: non-zero)
pub(crate) const PI_CAUSE: usize = 32; // last PI interrupt cause (bit 16 = reset button up)
pub(crate) const RESET_SEEN: usize = 36; // 1 once the reset button was seen pressed

/// Dolphin's core spec, or `None` (with a skip note) before its fetch script ran.
pub(crate) fn dolphin() -> Option<&'static CoreSpec> {
    let core = system_catalog::core_for_system(GC.id).unwrap();
    if core_path(core.id).exists() {
        Some(core)
    } else {
        eprintln!("skipped: run scripts/fetch-{}-core first", core.id);
        None
    }
}

/// A scratch system folder holding `dolphin-emu/Sys` (copied from beside the core
/// through the runner's `CoreAssets` path, as a launch does).
pub(crate) fn system_folder(core: &CoreSpec) -> tempfile::TempDir {
    let system = scratch("core-suite-dolphin-system");
    ensure_core_assets(core, system.path(), cores_dir_of(&core_path(core.id))).unwrap();
    system
}

/// The runner's options for `system` with `overrides` (e.g. the Null renderer).
pub(crate) fn options(system: &SystemSpec, overrides: &[(&str, &str)]) -> Vec<(String, String)> {
    let owned: Vec<(String, String)> = overrides
        .iter()
        .map(|&(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    runner_core_options::resolve(system, &owned)
}

/// Loads `rom` (repo-relative) on Dolphin: shared `system`, per-test `saves`.
pub(crate) fn load_dol(
    core: &CoreSpec,
    rom: &str,
    (system, saves): (&Path, &Path),
    options: &[(String, String)],
    provider: Option<&dyn HwRenderProvider>,
) -> CoreSession {
    load_session_in(core.id, &repo(rom), (system, saves), options, provider)
}

/// Big-endian word at `offset` of the MEM1 debug block (0 while Dolphin exposes no
/// MEM1 yet: before its first frame).
pub(crate) fn debug_word(session: &mut CoreSession, offset: usize) -> u32 {
    let at = DEBUG_BLOCK + offset;
    session.system_ram().map_or(0, |ram| {
        u32::from_be_bytes(ram[at..at + 4].try_into().unwrap())
    })
}
