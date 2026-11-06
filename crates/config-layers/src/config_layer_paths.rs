//! Where each config layer lives. The game layer path comes from
//! `ipc_protocol::game_config_path_for_game` (its single owner, shared with the
//! save folder name); this module builds only the global/system/core paths.

use std::path::{Path, PathBuf};

/// `<data>/config/global.toml`: defaults for every game (launcher Settings page).
pub fn global_config_path(data_root: &Path) -> PathBuf {
    ipc_protocol::config_dir(data_root).join("global.toml")
}

/// The four layer files for one game, lowest precedence first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLayerPaths {
    pub global: PathBuf,
    /// `system/<system>.toml`.
    pub system: PathBuf,
    /// `core/<core id>.toml`; `core_id` comes from [`crate::core_layer_id`].
    pub core: PathBuf,
    /// `game/<system>/<rom key> [<content id>].toml`.
    pub game: PathBuf,
}

impl ConfigLayerPaths {
    /// The layers of one game whose game layer file is `game` (the web edition finds
    /// it by listing the folder itself; see `ipc_protocol::pick_game_entry_name`).
    pub fn with_game_layer(data_root: &Path, system: &str, core_id: &str, game: PathBuf) -> Self {
        let config_dir = ipc_protocol::config_dir(data_root);
        Self {
            global: global_config_path(data_root),
            system: config_dir.join("system").join(format!("{system}.toml")),
            core: config_dir.join("core").join(format!("{core_id}.toml")),
            game,
        }
    }

    /// `content_sha1` = the game's stored sha1 (names the game layer file, looked up
    /// on disk).
    #[cfg(feature = "fs")]
    pub fn new(
        data_root: &Path,
        system: &str,
        core_id: &str,
        rom_path: &Path,
        content_sha1: &str,
    ) -> Self {
        let game =
            ipc_protocol::game_config_path_for_game(data_root, system, rom_path, content_sha1);
        Self::with_game_layer(data_root, system, core_id, game)
    }

    /// Merge order: each later layer overrides the earlier ones.
    pub fn lowest_first(&self) -> [&Path; 4] {
        [&self.global, &self.system, &self.core, &self.game]
    }
}

#[cfg(test)]
#[cfg(feature = "fs")]
mod tests {
    use super::*;

    #[test]
    fn layer_layout_under_config_dir() {
        let root = Path::new("data");
        let paths = ConfigLayerPaths::new(root, "snes", "snes9x", Path::new("roms/Game.sfc"), "");
        let config = root.join("config");
        assert_eq!(
            paths.lowest_first(),
            [
                config.join("global.toml").as_path(),
                config.join("system").join("snes.toml").as_path(),
                config.join("core").join("snes9x.toml").as_path(),
                config.join("game").join("snes").join("Game.toml").as_path(),
            ]
        );
    }
}
