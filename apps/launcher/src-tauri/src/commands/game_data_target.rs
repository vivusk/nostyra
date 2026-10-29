//! Where one game's per-game data lives, resolved exactly like the runner does:
//! config layers (system = the game's platform, core layer id from the catalog
//! core) and the save folder (`<rom key> [<content id>]` from the stored ROM path
//! and sha1). Shared by the gameplay-config and save-state commands.

use std::path::{Path, PathBuf};

use config_layers::{
    core_layer_id, resolve_config_with_warnings, ConfigLayerPaths, ResolvedConfig,
};

use crate::app_state::AppState;
use crate::command_error::{CommandError, CommandResult};

/// The stored facts about a game that its data paths depend on.
#[derive(Debug, Clone)]
pub struct GameDataTarget {
    /// Platform id: config `system/<id>.toml` and the saves subfolder.
    pub system: String,
    /// `games.rom_path` exactly as passed to the runner's `--rom`.
    pub rom_path: PathBuf,
    /// `games.sha1`: the content id in the save folder / game config names.
    pub content_sha1: String,
    /// ROM not found on the last scan.
    pub missing: bool,
}

impl GameDataTarget {
    /// Looks the game up; `invalid` when the id is unknown.
    pub fn load(state: &AppState, game_id: i64) -> CommandResult<Self> {
        let game = storage_db::get_game(&state.db(), game_id)?
            .ok_or_else(|| CommandError::invalid("This game is no longer in the library."))?;
        Ok(Self::from_game(&game))
    }

    /// The facts of an already loaded game row (Play).
    pub fn from_game(game: &storage_db::Game) -> Self {
        Self {
            system: game.platform_id.clone(),
            rom_path: game.rom_path.clone().into(),
            content_sha1: game.sha1.clone(),
            missing: game.missing,
        }
    }

    /// The four config layers the runner would read for this game. The core layer
    /// id comes from the registry core of the game's platform: the DB does not record
    /// the core's `library_name`, and the registry keeps `CoreSpec.id ==
    /// core_layer_id(library_name)` (e.g. "Snes9x" → `snes9x`, "Mesen" → `mesen`).
    pub fn config_layers(&self, data_root: &Path) -> ConfigLayerPaths {
        let core_id = system_catalog::core_for_system(&self.system).map_or("unknown", |c| c.id);
        ConfigLayerPaths::new(
            data_root,
            &self.system,
            &core_layer_id(core_id),
            &self.rom_path,
            &self.content_sha1,
        )
    }

    /// The typed effective config plus the binding-conflict warnings (Controls
    /// notice, resume mode, steal computations).
    pub fn resolve_with_warnings(&self, data_root: &Path) -> (ResolvedConfig, Vec<String>) {
        resolve_config_with_warnings(&self.config_layers(data_root), self.system_actions())
    }

    /// Pad actions of the game's system (input conflict rules: PS1 L2/R2 take the
    /// triggers from the hold hotkeys); `None` for an unknown system id.
    pub fn system_actions(&self) -> Option<&'static [&'static str]> {
        system_catalog::system(&self.system).map(|spec| spec.pad_actions)
    }

    /// The game's save folder, or `None` when there is no ROM to key it on (no
    /// stored path, or the ROM is missing): the game then has no save states.
    pub fn saves_dir(&self, data_root: &Path) -> Option<PathBuf> {
        (!self.missing && !self.rom_path.as_os_str().is_empty()).then(|| {
            ipc_protocol::saves_dir_for_game(
                data_root,
                &self.system,
                &self.rom_path,
                &self.content_sha1,
            )
        })
    }
}

/// `busy` when `running_game` (the active session's game) is `game_id`: the
/// runner may be writing the same files. Callers hold the session lock across
/// their file work, so a Play cannot start in between.
pub fn ensure_idle(running_game: Option<i64>, game_id: i64) -> CommandResult<()> {
    if running_game == Some(game_id) {
        return Err(CommandError::new(
            "busy",
            "Close the game first: it is running right now.",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "runner_config_parity_tests.rs"]
mod runner_config_parity_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn target(rom: &str, missing: bool) -> GameDataTarget {
        GameDataTarget {
            system: "snes".into(),
            rom_path: rom.into(),
            content_sha1: "6b47bb75d16514b6a476aa0c73a683a2a4c18765".into(),
            missing,
        }
    }

    #[test]
    fn paths_match_the_shared_layout() {
        let root = Path::new("data");
        let game = target("E:/ROMs/Chrono Trigger.sfc", false);
        let layers = game.config_layers(root);
        let config = root.join("config");
        assert_eq!(layers.core, config.join("core").join("snes9x.toml"));
        assert_eq!(
            layers.game,
            config
                .join("game")
                .join("snes")
                .join("Chrono Trigger [6B47BB75].toml")
        );
        assert_eq!(
            game.saves_dir(root),
            Some(
                root.join("saves")
                    .join("snes")
                    .join("Chrono Trigger [6B47BB75]")
            )
        );
    }

    #[test]
    fn no_saves_dir_without_a_usable_rom() {
        assert_eq!(target("", false).saves_dir(Path::new("data")), None);
        assert_eq!(target("a.sfc", true).saves_dir(Path::new("data")), None);
    }

    #[test]
    fn busy_only_for_the_running_game() {
        assert!(ensure_idle(None, 1).is_ok());
        assert!(ensure_idle(Some(2), 1).is_ok());
        assert_eq!(ensure_idle(Some(1), 1).unwrap_err().kind, "busy");
    }
}
