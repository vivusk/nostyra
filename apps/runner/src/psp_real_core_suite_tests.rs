//! Sony PSP on PPSSPP (`scripts/fetch-ppsspp-core.*`) with the self-written test
//! program (`test-assets/roms/src/build-nostyra-test-psp.py`). Software backend
//! (`ppsspp_backend=none`: no GL, every OS) for the five containers; one OpenGL smoke
//! run behind `gl_test_gate`. `CI` comes from `.cargo/config.toml` (never `set_var`
//! here). Several PPSSPP sessions per process are fine (M12 phase-00 spike), so the
//! suite shares the runner's test binary. The core's system files are installed once
//! into a shared system folder; saves (the memory stick) stay per session.
//!
//! The program: top half red (green while ✕ is held), bottom half blue with the
//! frame counter in green (yellow while the stick points right), a square tone, and
//! a boot-time AUTOSAVE to `PSP/SAVEDATA/NSTY00001DATA00/`. States are `Serialized`
//! and not frame-exact (emulator thread); the ELF's state is 72 MiB (64 MB RAM).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};
use std::sync::PoisonError;

use disc_image::test_psp_containers::dvd_chd;
use ipc_protocol::StateSlotId;
use libretro_host::CoreSession;
use system_catalog::{DEFAULT_MAX_STATE_BYTES, PSP};

use crate::core_system_assets::{cores_dir_of, ensure_core_assets};
use crate::dreamcast_real_core_checks_tests::top;
use crate::pre_boot_load_guard::PreBootLoadGuard;
use crate::real_core_session_loader::{gl_test_gate, load_session_in};
use crate::runner_held_input::HeldInput;
use crate::runner_real_core_suite::{core_path, repo, scratch, CORE_LOCK};
use crate::state_load_checks::check_state_size;
use crate::{runner_core_options, save_state_slots_core_tests, state_policy_core_tests};
use checks::{boot, check_frames_input_and_audio, check_memory_stick_save, run};

#[path = "psp_real_core_checks_tests.rs"]
mod checks;
/// GameCube / Wii on Dolphin (M14): same binary, same `CORE_LOCK`.
#[path = "gamecube_wii_real_core/mod.rs"]
mod gamecube_wii_real_core;

const ELF: &str = "test-assets/roms/nostyra-test-psp.elf";
const CONTAINERS: [&str; 3] = [
    "test-assets/roms/nostyra-test-psp.iso",
    "test-assets/roms/nostyra-test-psp.cso",
    "test-assets/roms/nostyra-test-psp/EBOOT.PBP",
];

#[test]
fn psp_suite_ppsspp() {
    let _lock = CORE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let core = system_catalog::core_for_system(PSP.id).unwrap();
    if !core_path(core.id).exists() {
        eprintln!("skipped: run scripts/fetch-{}-core first", core.id);
        return;
    }
    let system = scratch("core-suite-psp-system");
    ensure_core_assets(core, system.path(), cores_dir_of(&core_path(core.id))).unwrap();
    let software = runner_core_options::resolve(&PSP, &[("ppsspp_backend".into(), "none".into())]);
    let load = |rom: &Path, saves: &Path| {
        load_session_in(core.id, rom, (system.path(), saves), &software, None)
    };
    {
        eprintln!("ppsspp suite: test program (ELF)");
        let saves = scratch("core-suite-psp-elf");
        let mut session = load(&repo(ELF), saves.path());
        check_pre_boot_guard(&mut session);
        check_frames_input_and_audio(&mut session);
        check_memory_stick_save(saves.path());
        check_state_size_and_slots(&mut session, saves.path());
        check_reset_reboots(&mut session, saves.path());
    }
    let chd_dir = scratch("core-suite-psp-chd");
    let chd = chd_dir.path().join("nostyra-test-psp.chd");
    std::fs::write(
        &chd,
        dvd_chd(&std::fs::read(repo(CONTAINERS[0])).unwrap(), true),
    )
    .unwrap();
    let roms: Vec<PathBuf> = CONTAINERS
        .iter()
        .map(|rel| repo(rel))
        .chain([chd])
        .collect();
    for rom in &roms {
        eprintln!("ppsspp suite: {}", rom.display());
        let saves = scratch("core-suite-psp-disc");
        let mut session = load(rom, saves.path());
        boot(&mut session, 30);
        check_frames_input_and_audio(&mut session);
        check_memory_stick_save(saves.path());
    }
    check_gl_smoke(core.id, system.path(), &repo(CONTAINERS[0]));
}

/// L6: before the asynchronous boot a slot load is refused; after it, allowed.
fn check_pre_boot_guard(session: &mut CoreSession) {
    let guard = PreBootLoadGuard::new(session);
    assert!(!guard.ready(session), "not booted right after the load");
    boot(session, 30);
    assert!(guard.ready(session), "booted");
}

/// The ELF's 72 MiB state passes the PPSSPP cap (128 MiB), not the default (64 MiB);
/// slots round-trip; run-ahead and rewind are refused.
fn check_state_size_and_slots(session: &mut CoreSession, saves: &Path) {
    let size = session.current_state_size() as u64;
    assert!(size > DEFAULT_MAX_STATE_BYTES, "64 MB RAM program: {size}");
    let slot = StateSlotId::Numbered(1);
    let cap = runner_core_options::max_state_bytes(&PSP);
    assert!(check_state_size(slot, size, cap).is_ok());
    assert!(check_state_size(slot, size, DEFAULT_MAX_STATE_BYTES).is_err());
    let states = saves.join("states");
    std::fs::create_dir_all(&states).unwrap();
    save_state_slots_core_tests::check_state_slots(session, &states, false);
    let quality = runner_core_options::savestate_quality(&PSP);
    state_policy_core_tests::check_states_refused(session, quality);
}

/// `retro_reset` reboots the program: the boot-time AUTOSAVE runs again. `DATA.BIN` is
/// deleted first, so only a real reboot can bring it back (the picture is the same
/// before and after, it cannot tell).
fn check_reset_reboots(session: &mut CoreSession, saves: &Path) {
    let data = saves.join("PSP/SAVEDATA/NSTY00001DATA00/DATA.BIN");
    std::fs::remove_file(&data).unwrap();
    session.reset();
    let back = (0..600).any(|_| {
        std::thread::sleep(std::time::Duration::from_millis(2));
        run(session, HeldInput::None, 1);
        data.is_file()
    });
    assert!(back, "autosave rewritten after reset: {}", data.display());
}

/// The UMD through PPSSPP's OpenGL backend: a context is used and the read-back
/// picture is upright at 2× (960×540).
fn check_gl_smoke(core_id: &str, system: &Path, iso: &Path) {
    if !gl_test_gate() {
        return;
    }
    eprintln!("ppsspp suite: OpenGL smoke");
    let provider = hw_render_gl::SdlGlProvider::new();
    let options = runner_core_options::resolve(&PSP, &[]);
    let saves = scratch("core-suite-psp-gl");
    let mut session = load_session_in(
        core_id,
        iso,
        (system, saves.path()),
        &options,
        Some(&provider),
    );
    assert!(session.uses_hw_context(), "OpenGL Core backend");
    boot(&mut session, 60);
    let (video, _) = run(&mut session, HeldInput::None, 4);
    assert_eq!(video.size(), (960, 540));
    let [r, g, b] = top(&video);
    assert!(
        r > 200 && g < 50 && b < 50,
        "GL top red (upright): {:?}",
        [r, g, b]
    );
}
