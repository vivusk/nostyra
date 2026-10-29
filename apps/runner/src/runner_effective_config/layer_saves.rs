//! Pause-menu config writes: "Save for this game" / "Save as default" write only
//! the menu option keys whose live value differs from what the files + CLI
//! resolve to, and "Reset game settings" removes the menu option keys from the game
//! layer. Controls / resume keys (launcher-edited) and unknown keys stay.
//! Each write re-resolves, so "changed" always means "differs from what the next
//! launch would use".

use config_layers::{changed_keys, set_layer_value, ConfigError, ResolvedConfig, MENU_OPTION_KEYS};

use super::EffectiveConfig;

/// Which layer a menu save writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveTarget {
    /// `game/<system>/<rom key> [<ID>].toml`.
    Game,
    /// `global.toml`.
    Global,
}

impl EffectiveConfig {
    /// Writes the keys `live` changed to `target`'s layer; returns how many.
    pub fn save_changes(
        &mut self,
        live: &ResolvedConfig,
        target: SaveTarget,
    ) -> Result<usize, ConfigError> {
        let changes: Vec<_> = changed_keys(&self.resolved, live)
            .into_iter()
            .filter(|(key, _)| MENU_OPTION_KEYS.contains(&key.as_str()))
            .collect();
        let path = match target {
            SaveTarget::Game => self.layers.game.clone(),
            SaveTarget::Global => self.layers.global.clone(),
        };
        let written = changes.len();
        let result = changes
            .into_iter()
            .try_for_each(|(key, value)| set_layer_value(&path, &key, Some(value)));
        self.reload(); // also after a partial write: keep `resolved` true to the files
        result.map(|()| written)
    }

    /// Removes every menu option key from the game layer and re-resolves.
    pub fn reset_game_layer(&mut self) -> Result<(), ConfigError> {
        let result = MENU_OPTION_KEYS
            .iter()
            .try_for_each(|key| set_layer_value(&self.layers.game, key, None));
        self.reload();
        result
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)] // test code: panics are the failure signal

    use std::path::Path;

    use config_layers::{global_config_path, ConfigLayerPaths, Value, VideoScaling};

    use super::super::CliOverrides;
    use super::*;

    fn scratch(name: &str) -> tempfile::TempDir {
        let prefix = format!("nostyra-menu-config-{name}-");
        tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
    }

    fn config_in(root: &Path, cli: CliOverrides) -> EffectiveConfig {
        let layers = ConfigLayerPaths::new(root, "snes", "snes9x", Path::new("roms/Game.sfc"), "");
        EffectiveConfig::from_layers(layers, cli, system_catalog::SNES.pad_actions)
    }

    #[test]
    fn saves_only_changed_keys_to_the_chosen_layer() {
        let temp = scratch("save");
        let root = temp.path();
        let global = global_config_path(root);
        set_layer_value(
            &global,
            "video.shader",
            Some(Value::String("crt-geom".into())),
        )
        .unwrap();
        let mut config = config_in(root, CliOverrides::default());
        assert_eq!(config.resolved().video.shader, "crt-geom");

        let mut live = config.resolved().clone();
        live.video.scaling = VideoScaling::Integer;
        assert_eq!(config.save_changes(&live, SaveTarget::Game).unwrap(), 1);
        let game = std::fs::read_to_string(&config.layers.game).unwrap();
        assert!(
            game.contains("scaling = \"integer\"") && !game.contains("shader"),
            "{game}"
        );
        assert_eq!(
            config.resolved().video.scaling,
            VideoScaling::Integer,
            "re-resolved"
        );
        assert_eq!(
            config.save_changes(&live, SaveTarget::Game).unwrap(),
            0,
            "nothing left"
        );

        live.latency.run_ahead_frames = 1;
        assert_eq!(config.save_changes(&live, SaveTarget::Global).unwrap(), 1);
        let global_text = std::fs::read_to_string(&global).unwrap();
        assert!(global_text.contains("run_ahead_frames = 1") && global_text.contains("crt-geom"));
        assert!(
            !global_text.contains("scaling"),
            "game-only change stays out of global"
        );
    }

    #[test]
    fn reset_removes_menu_option_keys_from_the_game_layer_only() {
        let temp = scratch("reset");
        let root = temp.path();
        let mut config = config_in(root, CliOverrides::default());
        let game = config.layers.game.clone();
        set_layer_value(&game, "rewind.enabled", Some(Value::Boolean(true))).unwrap();
        set_layer_value(
            &game,
            "video.custom_note",
            Some(Value::String("keep".into())),
        )
        .unwrap();
        let north = Value::Array(vec![Value::from("north")]);
        set_layer_value(&game, "input_gamepad.a", Some(north)).unwrap();
        set_layer_value(&game, "resume.mode", Some(Value::from("never"))).unwrap();
        config.reload();
        assert!(config.resolved().rewind.enabled);
        config.reset_game_layer().unwrap();
        assert!(!config.resolved().rewind.enabled, "back to the default");
        let text = std::fs::read_to_string(&game).unwrap();
        assert!(
            text.contains("custom_note") && !text.contains("enabled"),
            "{text}"
        );
        assert_eq!(config.resolved().input.gamepad["a"], ["north"], "{text}");
        assert_eq!(
            config.resolved().resume.mode,
            config_layers::ResumeMode::Never
        );
    }

    #[test]
    fn saves_never_write_control_or_resume_keys() {
        let temp = scratch("menu-only");
        let root = temp.path();
        let mut config = config_in(root, CliOverrides::default());
        let mut live = config.resolved().clone();
        live.input.keyboard.insert("b".into(), Vec::new());
        live.resume.auto_save = false;
        assert_eq!(config.save_changes(&live, SaveTarget::Game).unwrap(), 0);
        assert!(!config.layers.game.exists());
    }

    #[test]
    fn cli_flags_are_not_written_unless_changed_and_still_win() {
        let temp = scratch("cli");
        let root = temp.path();
        let cli = CliOverrides {
            run_ahead: Some(2),
            ..CliOverrides::default()
        };
        let mut config = config_in(root, cli);
        let mut live = config.resolved().clone();
        live.rewind.enabled = true;
        assert_eq!(config.save_changes(&live, SaveTarget::Game).unwrap(), 1);
        let game = std::fs::read_to_string(&config.layers.game).unwrap();
        assert!(
            !game.contains("run_ahead"),
            "CLI value not persisted: {game}"
        );
        assert_eq!(config.resolved().latency.run_ahead_frames, 2);
    }
}
