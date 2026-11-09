//! Loading a test ROM on the pinned core of an RA system the way the achievement
//! suites need it (M15 phases 02 / 07): the registry's fixed options plus the test
//! overrides (PCSX ReARMed HLE BIOS, PPSSPP software backend + system files,
//! Mupen64Plus-Next on Angrylion without OpenGL, PCEE2 with the user's BIOS copied
//! in), N64 / Dreamcast on the GL provider when the GL gate passed, Dreamcast skipped
//! without it, PS2 skipped without `NOSTYRA_PS2_BIOS_DIR`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::Path;

use libretro_host::{CoreSession, HwRenderProvider, NullAudioSink, NullVideoSink};

use crate::core_system_assets::{cores_dir_of, ensure_core_assets};
use crate::real_core_session_loader::{core_path, load_session_in};
use crate::runner_core_options;
use crate::runner_held_input::HeldInput;
use crate::runner_real_core_suite::scratch;

/// PS2 needs the user's BIOS in the system folder (`NOSTYRA_PS2_BIOS_DIR`); the
/// PS2 suite shares this one include (clippy: a file is a module once).
#[path = "ps2_bios_test_gate.rs"]
pub(crate) mod ps2_bios_gate;

/// A loaded core session with its folders (dropped after the session: the core keeps
/// files in them open on Windows) and the options the host answers.
pub(crate) struct RaCoreSession {
    pub(crate) session: CoreSession,
    pub(crate) options: Vec<(String, String)>,
    _saves: tempfile::TempDir,
    _system: tempfile::TempDir,
}

/// Loads `rom` for `system_id`; `Err(reason)` when the case must be skipped here.
pub(crate) fn load_ra_core(
    system_id: &str,
    rom: &Path,
    gl: Option<&dyn HwRenderProvider>,
) -> Result<RaCoreSession, &'static str> {
    let spec = system_catalog::system(system_id).unwrap();
    let core = system_catalog::core_for_system(system_id).unwrap();
    if !core_path(core.id).exists() {
        return Err("core missing (run its scripts/fetch-*-core script)");
    }
    let (system_dir, saves) = (scratch("ra-core-system"), scratch("ra-core-saves"));
    let mut overrides: Vec<(String, String)> = Vec::new();
    match system_id {
        "psx" => overrides.push(("pcsx_rearmed_bios".into(), "HLE".into())),
        "psp" => {
            overrides.push(("ppsspp_backend".into(), "none".into()));
            ensure_core_assets(core, system_dir.path(), cores_dir_of(&core_path(core.id))).unwrap();
        }
        "ps2" => {
            // No HLE BIOS: without the user's dump the case skips (phase-06 gate).
            ps2_bios_gate::install_user_bios(system_dir.path())?;
        }
        "n64" if gl.is_none() => {
            overrides.push(("mupen64plus-rdp-plugin".into(), "angrylion".into()))
        }
        "dreamcast" if gl.is_none() => return Err("no OpenGL 3.3 (Flycast needs it)"),
        _ => {}
    }
    let gl = matches!(system_id, "n64" | "dreamcast")
        .then_some(gl)
        .flatten();
    let options = runner_core_options::resolve(spec, &overrides);
    let session = load_session_in(
        core.id,
        rom,
        (system_dir.path(), saves.path()),
        &options,
        gl,
    );
    Ok(RaCoreSession {
        session,
        options,
        _saves: saves,
        _system: system_dir,
    })
}

/// One real frame without video, audio or input.
pub(crate) fn run_quiet_frame(session: &mut CoreSession) {
    let (mut video, mut audio) = (NullVideoSink::default(), NullAudioSink::default());
    session
        .run_frame(&mut video, &mut audio, &mut HeldInput::None)
        .unwrap();
}
