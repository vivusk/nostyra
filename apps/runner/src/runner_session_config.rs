//! Per-core setup around `CoreSession::load`, all from the registry (`CoreSpec`):
//! before the load the core's system files and process environment, after it the
//! hardware-render rules (a refused request is fatal for `hw_render_required` cores,
//! their states need the context), state-size rules and controller ports (library
//! retention, `keep_library_loaded`, is part of the load itself: `SessionConfig`).
//! Shared by the window and headless runs.

use std::path::Path;

use libretro_host::{CoreSession, HostError};
use system_catalog::{CoreSpec, SystemSpec};

use crate::core_system_assets::{cores_dir_of, ensure_core_assets};
use crate::runner_core_options;

/// Before the load: the core's system files in `system_dir` (installed from beside
/// `core_path`; missing → `MissingCoreAssets`, exit 3), then its environment
/// variables (`set_process_env`: also msvcrt's copy for MinGW cores). Must run before SDL or any other thread starts: setting the
/// environment is only sound while no thread can read it concurrently.
pub fn prepare_core(
    system: &SystemSpec,
    core_path: &Path,
    system_dir: &Path,
) -> anyhow::Result<()> {
    let Some(core) = system_catalog::core_for_system(system.id) else {
        return Ok(());
    };
    ensure_core_assets(core, system_dir, cores_dir_of(core_path))?;
    for (key, value) in process_env(core) {
        tracing::debug!(key, value, "core process environment");
        libretro_host::set_process_env(key, value);
    }
    Ok(())
}

/// Environment variables the runner sets for `core` (PPSSPP: `CI`, so a release
/// assert logs instead of opening a modal dialog).
pub fn process_env(core: &CoreSpec) -> &'static [(&'static str, &'static str)] {
    core.process_env
}

/// Exit-6 reason when a `hw_render_required` core loaded without a graphics context.
/// Generic on purpose: the refused request named last is not the one that matters
/// (Dolphin tries OpenGL first and Vulkan last, M14 spike).
pub const HW_RENDER_REQUIRED: &str =
    "this core needs OpenGL 3.3 hardware rendering, which this PC could not provide";

/// After the load: the registry's hardware-render rules (`Err` = exit 6, the core
/// is abandoned: no unload of a core running its black Null renderer), state-size
/// rules and controller port devices.
pub fn configure_session(session: &mut CoreSession, system: &SystemSpec) -> anyhow::Result<()> {
    let core = system_catalog::core_for_system(system.id);
    let required = core.is_some_and(|core| core.hw_render_required);
    let refused = session.refused_hw_render().is_some();
    if let Some(err) = fatal_hw_refusal(required, refused, session.uses_hw_context()) {
        session.abandon_core();
        return Err(err.into());
    }
    // Dolphin's states segfault without a context (renderer=Null in tests).
    session.require_hw_context_for_states(required);
    // Mupen64Plus-Next hangs on a bad state: wrong sizes never reach it.
    session.set_exact_state_size(runner_core_options::exact_state_size(system));
    // PPSSPP's state size changes after boot without saying so.
    session.set_state_size_requery(runner_core_options::state_size_requery(system));
    session.set_controller_port_devices(runner_core_options::port_devices(system));
    // PPSSPP falls back to its software renderer without a word when every
    // hardware request is refused (its own "Using … backend" log line says which).
    if let Some(reason) = session.refused_hw_render() {
        if !session.uses_hw_context() {
            tracing::warn!(reason, "core renders in software: graphics request refused");
        }
    }
    Ok(())
}

/// The exit-6 error when a core that must render on the GPU (`required`) had a
/// request refused (`refused`) and runs without a context.
fn fatal_hw_refusal(required: bool, refused: bool, has_context: bool) -> Option<HostError> {
    (required && refused && !has_context)
        .then(|| HostError::HwRenderUnavailable(HW_RENDER_REQUIRED.to_owned()))
}

#[cfg(test)]
mod tests {
    use system_catalog::CORE_DEFAULTS;

    use super::*;

    #[test]
    fn environment_comes_from_the_core_spec() {
        const WITH_ENV: CoreSpec = CoreSpec {
            process_env: &[("CI", "1")],
            ..CORE_DEFAULTS
        };
        assert_eq!(process_env(&WITH_ENV), [("CI", "1")]);
        assert!(process_env(&system_catalog::SNES9X).is_empty());
    }

    #[test]
    fn refusal_is_fatal_only_for_flagged_cores_without_a_context() {
        let err = fatal_hw_refusal(true, true, false).expect("exit 6");
        assert!(matches!(&err, HostError::HwRenderUnavailable(_)));
        let text = err.to_string();
        assert!(
            text.contains("OpenGL 3.3") && !text.contains("Vulkan"),
            "{text}"
        );
        let code = crate::runner_exit_codes::exit_code_for(&err.into());
        assert_eq!(code, crate::runner_exit_codes::EXIT_GRAPHICS);
        assert!(
            fatal_hw_refusal(false, true, false).is_none(),
            "PPSSPP: warning"
        );
        assert!(
            fatal_hw_refusal(true, true, true).is_none(),
            "a later request won"
        );
        assert!(
            fatal_hw_refusal(true, false, false).is_none(),
            "renderer=Null asks nothing"
        );
    }

    #[test]
    fn systems_without_extra_needs_prepare_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let core = temp.path().join("cores/snes9x_libretro.dll");
        prepare_core(&system_catalog::SNES, &core, temp.path()).unwrap();
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }
}
