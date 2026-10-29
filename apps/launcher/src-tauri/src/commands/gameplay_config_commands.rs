//! Gameplay settings (shader, scaling, run-ahead, rewind, resume) and the controls
//! view: read the effective config and the global / game layers, and set or clear
//! one key of a layer. Values travel as flat dotted-key maps (`{"video.shader":
//! "crt-geom"}`), the same keys the TOML layers and `set_layer_value` use. Binding
//! keys live in `view.input` only, so the gameplay form (and its "Reset all") never
//! sees them. System/core layers are file-only.

use std::path::{Path, PathBuf};

use config_layers::{
    global_config_path, known_keys, layer_known_values, resolve_config_with_warnings,
    set_layer_value, ConfigLayerPaths, ResolvedConfig, Table, Value, BUNDLED_PRESETS,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::command_error::{CommandError, CommandResult};
use crate::commands::blocking::run_blocking;
use crate::commands::game_data_target::GameDataTarget;
use crate::commands::input_config_view::{
    is_global_only_key, resolve_global_only, split_game_layer, split_input_keys, split_resolved,
    InputConfigView,
};

/// What the Settings page (no game) or the Detail Settings tab (a game) shows.
#[derive(Debug, Serialize)]
pub struct GameplayConfigView {
    /// Every gameplay key (menu options + `resume.*`) with the value the runner /
    /// launcher will use (CLI flags aside).
    pub resolved: Table,
    /// Gameplay keys the game layer sets (empty without a game).
    pub game: Table,
    /// Gameplay keys the global layer sets.
    pub global: Table,
    /// Bundled shader presets besides `none`.
    pub presets: &'static [&'static str],
    /// The binding sections (Controls editor), same layering.
    pub input: InputConfigView,
}

/// Which layer a write goes to: `"global"` or `{ "gameId": n }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ConfigScope {
    Game {
        #[serde(rename = "gameId")]
        game_id: i64,
    },
    Global(GlobalScope),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GlobalScope {
    Global,
}

/// Effective config for `game_id` (all layers, as the runner resolves them) or,
/// without a game, the global layer resolved over the built-in defaults.
#[tauri::command]
pub async fn get_gameplay_config(
    app: AppHandle,
    game_id: Option<i64>,
) -> CommandResult<GameplayConfigView> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let data_root = &state.paths.data_root;
        let target = game_id
            .map(|game_id| GameDataTarget::load(&state, game_id))
            .transpose()?;
        let game_layers = target.as_ref().map(|t| t.config_layers(data_root));
        let system_actions = target.as_ref().and_then(GameDataTarget::system_actions);
        Ok(build_view(
            &global_config_path(data_root),
            game_layers.as_ref(),
            system_actions,
        ))
    })
    .await
}

/// The view for a game's layers (with its system's pad actions), or (`None`) for the
/// global layer alone. Its `resolved` + `input.resolved` are the launcher's half of
/// the runner `--print-config` parity.
pub(crate) fn build_view(
    global_path: &Path,
    game_layers: Option<&ConfigLayerPaths>,
    system_actions: Option<&[&str]>,
) -> GameplayConfigView {
    let (global, input_global) = split_input_keys(layer_view(global_path));
    let ((config, warnings), (game, input_game)) = match game_layers {
        Some(layers) => (
            resolve_config_with_warnings(layers, system_actions),
            split_game_layer(layer_view(&layers.game)),
        ),
        None => (resolve_global_only(global_path), Default::default()),
    };
    let (resolved, input_resolved) = split_resolved(&config);
    GameplayConfigView {
        resolved,
        game,
        global,
        presets: BUNDLED_PRESETS,
        input: InputConfigView {
            resolved: input_resolved,
            game: input_game,
            global: input_global,
            warnings,
        },
    }
}

/// Sets (`value`) or clears (`null` = "use default") one known key (gameplay or a
/// single binding, value = array of input names) in the global or a game's layer.
/// Unknown keys and out-of-range values are refused (`invalid`).
#[tauri::command]
pub async fn set_gameplay_config_value(
    app: AppHandle,
    scope: ConfigScope,
    key: String,
    value: Option<Value>,
) -> CommandResult<()> {
    run_blocking(move || {
        refuse_global_only_in_game(&scope, &key)?;
        let state = app.state::<AppState>();
        let path = scope_layer_path(&state, &scope)?;
        write_known_value(&path, &key, value)
    })
    .await
}

/// `achievements.*` is global-only: refuse a game write rather than ignore it.
fn refuse_global_only_in_game(scope: &ConfigScope, key: &str) -> CommandResult<()> {
    if matches!(scope, ConfigScope::Game { .. }) && is_global_only_key(key) {
        return Err(CommandError::invalid(format!("`{key}` is global-only.")));
    }
    Ok(())
}

/// The layer file a write to `scope` edits.
pub fn scope_layer_path(state: &AppState, scope: &ConfigScope) -> CommandResult<PathBuf> {
    let data_root = &state.paths.data_root;
    Ok(match scope {
        ConfigScope::Global(GlobalScope::Global) => global_config_path(data_root),
        ConfigScope::Game { game_id } => {
            GameDataTarget::load(state, *game_id)?
                .config_layers(data_root)
                .game
        }
    })
}

/// The layer a write to `scope` edits plus the effective config that scope sees
/// (global: global over defaults; game: all layers), for multi-key edits.
pub fn scope_layer_and_config(
    state: &AppState,
    scope: &ConfigScope,
) -> CommandResult<(PathBuf, ResolvedConfig)> {
    let data_root = &state.paths.data_root;
    Ok(match scope {
        ConfigScope::Global(GlobalScope::Global) => {
            let path = global_config_path(data_root);
            let (config, _) = resolve_global_only(&path);
            (path, config)
        }
        ConfigScope::Game { game_id } => {
            let target = GameDataTarget::load(state, *game_id)?;
            let (config, _) = target.resolve_with_warnings(data_root);
            (target.config_layers(data_root).game, config)
        }
    })
}

/// Writes one known key; anything outside the schema is refused before touching
/// the file (`set_layer_value` itself accepts unknown keys).
fn write_known_value(layer: &Path, key: &str, value: Option<Value>) -> CommandResult<()> {
    if !known_keys().any(|known| known == key) {
        return Err(CommandError::invalid(format!(
            "Unknown gameplay setting `{key}`."
        )));
    }
    set_layer_value(layer, key, value)?;
    tracing::info!(layer = %layer.display(), key, "gameplay setting saved");
    Ok(())
}

/// One layer's known keys; an unreadable or non-TOML file shows as empty (the
/// runner skips it too) and a later write reports the problem.
fn layer_view(path: &Path) -> Table {
    layer_known_values(path).unwrap_or_else(|err| {
        tracing::warn!("config layer not shown: {err}");
        Table::new()
    })
}

#[cfg(test)]
#[path = "gameplay_config_commands_tests.rs"]
mod tests;
