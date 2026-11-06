//! Desktop-only (feature `native`): the portable-copy check and the per-game entry
//! paths, which look on disk (and may rename a legacy entry) through
//! `resolve_game_entry`. The OS default data root lives in `default_data_root`.

use std::path::{Path, PathBuf};

use crate::game_folder_key::resolve_game_entry;
use crate::shared_data_paths::{
    game_config_system_dir, saves_system_dir, GAME_CONFIG_EXT, PORTABLE_MARKER_FILE,
};

/// The folder of the running exe when it holds [`PORTABLE_MARKER_FILE`] (portable
/// copy); `None` for an installed or dev build. Launcher and runner sit side by side,
/// so both see the same marker.
pub fn portable_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    portable_dir_of(exe.parent()?)
}

/// [`portable_dir`] for a given exe folder (pure, for tests).
pub fn portable_dir_of(exe_dir: &Path) -> Option<PathBuf> {
    exe_dir
        .join(PORTABLE_MARKER_FILE)
        .is_file()
        .then(|| exe_dir.to_path_buf())
}

/// The game's own folder: SRAM, save-state slots and the core's save directory.
/// `content_sha1` is the game's stored sha1 (`games.sha1`); see `resolve_game_entry`
/// (may rename a legacy folder; never creates one).
pub fn saves_dir_for_game(
    data_root: &Path,
    system: &str,
    rom_path: &Path,
    content_sha1: &str,
) -> PathBuf {
    resolve_game_entry(
        &saves_system_dir(data_root, system),
        rom_path,
        content_sha1,
        "",
    )
}

/// The game config layer file. The only place this path is built: config-layers
/// and the launcher both take it from here.
pub fn game_config_path_for_game(
    data_root: &Path,
    system: &str,
    rom_path: &Path,
    content_sha1: &str,
) -> PathBuf {
    resolve_game_entry(
        &game_config_system_dir(data_root, system),
        rom_path,
        content_sha1,
        GAME_CONFIG_EXT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_only_with_the_marker_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(portable_dir_of(dir.path()), None);
        std::fs::write(dir.path().join(PORTABLE_MARKER_FILE), "").unwrap();
        assert_eq!(portable_dir_of(dir.path()), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn per_game_paths_share_the_name() {
        let root = Path::new("no-such-data-dir");
        let rom = Path::new("E:/ROMs/Zelda: Link?.sfc");
        let sha1 = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";
        assert_eq!(
            saves_dir_for_game(root, "snes", rom, sha1),
            root.join("saves")
                .join("snes")
                .join("Zelda_ Link_ [6B47BB75]")
        );
        assert_eq!(
            game_config_path_for_game(root, "snes", rom, sha1),
            root.join("config")
                .join("game")
                .join("snes")
                .join("Zelda_ Link_ [6B47BB75].toml")
        );
    }
}
