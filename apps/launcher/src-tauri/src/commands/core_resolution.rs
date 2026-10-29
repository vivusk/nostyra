//! Which libretro core plays a game: the system registry's core for the game's
//! platform (`system_catalog::core_for_system`). No core or system is named in
//! launcher code; adding a system only touches the registry.

use std::path::{Path, PathBuf};

use system_catalog::CoreSpec;

use crate::command_error::{CommandError, CommandResult};

/// Core for `platform_id`; `invalid` for a platform outside the registry.
pub fn core_for_platform(platform_id: &str) -> CommandResult<&'static CoreSpec> {
    system_catalog::core_for_system(platform_id).ok_or_else(|| {
        CommandError::invalid(format!("No emulator core plays \"{platform_id}\" games."))
    })
}

/// A registry core by id (`invalid` otherwise: the UI never sends free text here).
pub fn core_by_id(core_id: &str) -> CommandResult<&'static CoreSpec> {
    system_catalog::core(core_id)
        .ok_or_else(|| CommandError::invalid(format!("Unknown emulator core \"{core_id}\".")))
}

/// Core for `platform_id` and its installed library, or a `core_missing` error
/// carrying the core id (the UI offers to install it).
pub fn installed_core_for_platform(
    cores_dir: &Path,
    platform_id: &str,
) -> CommandResult<(&'static CoreSpec, PathBuf)> {
    let core = core_for_platform(platform_id)?;
    match core_manager::installed_core(cores_dir, core.id) {
        Some(path) => Ok((core, path)),
        None => Err(CommandError::core_missing(core, platform_id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_core_names_the_core_and_system() {
        let temp = tempfile::Builder::new()
            .prefix("nostyra-core-resolve-")
            .tempdir()
            .unwrap();
        // Only snes9x installed: NES games need their own core.
        let snes9x = core_manager::current_platform_core_source("snes9x").unwrap();
        std::fs::write(temp.path().join(snes9x.library_file_name), b"stub").unwrap();
        let (core, path) = installed_core_for_platform(temp.path(), "snes").unwrap();
        assert_eq!(core.id, "snes9x");
        assert!(path.is_file());
        let err = installed_core_for_platform(temp.path(), "nes").unwrap_err();
        assert_eq!(err.kind, "core_missing");
        assert_eq!(err.core_id, Some("mesen"));
        assert!(err.message.contains("NES"), "{}", err.message);
        assert_eq!(
            core_for_platform("no-such-system").unwrap_err().kind,
            "invalid"
        );
        assert_eq!(core_by_id("../evil").unwrap_err().kind, "invalid");
    }
}
