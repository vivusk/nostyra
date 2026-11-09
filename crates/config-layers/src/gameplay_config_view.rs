//! What the launcher's gameplay forms and Controls editor show for one scope, built
//! from already-resolved values: the gameplay part (menu options + `resume.*` +
//! `achievements.*`, what "Reset all to defaults" acts on) and the controls part
//! (the four binding sections), so a gameplay reset can never touch a binding. Pure:
//! the desktop feeds it from layer files, the web edition from layer texts.

use serde::Serialize;
use toml::{Table, Value};

use crate::cloud_config::CLOUD_COMMAND_ONLY_KEYS;
use crate::core_layer_id::BUNDLED_PRESETS;
use crate::input_binding_names::INPUT_KEY_PREFIXES;
use crate::known_values_view::resolved_known_values;
use crate::resolved_config_diff::known_keys;
use crate::resolved_config_schema::ResolvedConfig;

/// The Controls editor's data: flat dotted binding keys (`"input_keyboard.b":
/// ["KeyZ"]`) per layer, like the gameplay view.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct InputConfigView {
    /// Every binding key with the binds the runner will use (conflicts resolved).
    pub resolved: Table,
    /// Binding keys the game layer sets (empty without a game).
    pub game: Table,
    /// Binding keys the global layer sets.
    pub global: Table,
    /// Cross-key conflicts found after merging (hotkey wins), for a notice.
    pub warnings: Vec<String>,
}

/// What the Settings page (no game) or the Detail Settings tab (a game) shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GameplayConfigView {
    /// Every gameplay key with the value the runner / launcher will use.
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

/// Builds the view of one scope: `config` + `warnings` as resolved for it, and the
/// known values the global and game layers set (`game` empty without a game).
pub fn gameplay_config_view(
    config: &ResolvedConfig,
    warnings: Vec<String>,
    global_layer: Table,
    game_layer: Table,
) -> GameplayConfigView {
    let (global, input_global) = split_input_keys(global_layer);
    let (game, input_game) = split_game_layer(game_layer);
    let (resolved, input_resolved) = split_input_keys(resolved_known_values(config));
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

/// Whether a dotted key belongs to one of the binding sections.
pub fn is_input_key(key: &str) -> bool {
    INPUT_KEY_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
}

/// Whether a dotted key is global-layer only (`[achievements]`, `[cloud]`).
pub fn is_global_only_key(key: &str) -> bool {
    key.starts_with("achievements.") || key.starts_with("cloud.")
}

/// Edits removing every binding key (gameplay and resume keys stay): "Reset controls".
pub fn reset_controls_edits() -> Vec<(String, Option<Value>)> {
    known_keys()
        .filter(|key| is_input_key(key))
        .map(|key| (key, None))
        .collect()
}

/// Why a single-key write of `key` is refused (`None` = allowed): unknown keys,
/// global-only keys in a game layer (refused rather than silently ignored), and the
/// cloud provider / folder (only the validating cloud commands set those).
pub fn settable_key_refusal(key: &str, game_scope: bool) -> Option<String> {
    if CLOUD_COMMAND_ONLY_KEYS.contains(&key) {
        return Some(format!(
            "`{key}` is changed in Settings → Cloud saves only."
        ));
    }
    if game_scope && is_global_only_key(key) {
        return Some(format!("`{key}` is global-only."));
    }
    if !known_keys().any(|known| known == key) {
        return Some(format!("Unknown gameplay setting `{key}`."));
    }
    None
}

/// `(gameplay keys, binding keys)` of one flat view.
fn split_input_keys(table: Table) -> (Table, Table) {
    table.into_iter().partition(|(key, _)| !is_input_key(key))
}

/// A game layer's `(gameplay, binding)` keys without the global-only ones, which the
/// runner ignores there (so the per-game form never offers or resets them).
fn split_game_layer(mut table: Table) -> (Table, Table) {
    table.retain(|key, _| !is_global_only_key(key));
    split_input_keys(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_view_splits_controls_from_gameplay() {
        let view = gameplay_config_view(
            &ResolvedConfig::default(),
            vec![],
            Table::new(),
            Table::new(),
        );
        assert!(view.resolved.keys().all(|key| !is_input_key(key)));
        assert!(
            view.resolved.contains_key("video.shader") && view.resolved.contains_key("resume.mode")
        );
        assert_eq!(
            view.resolved.len(),
            16,
            "5 menu options + 2 resume + 5 achievements + 4 cloud keys"
        );
        assert_eq!(
            view.input.resolved.len(),
            48 + 16,
            "2 × 24 actions + 2 × 8 hotkeys"
        );
        assert!(view.input.resolved.keys().all(|key| is_input_key(key)));
        assert!(!is_input_key("input.keyboard") && is_input_key("hotkeys_gamepad.save_state"));
    }

    #[test]
    fn game_layer_drops_global_only_keys() {
        let mut game = Table::new();
        game.insert("achievements.enabled".into(), Value::Boolean(true));
        game.insert("video.shader".into(), Value::from("crt-geom"));
        game.insert("input_keyboard.a".into(), Value::Array(vec![]));
        let view = gameplay_config_view(&ResolvedConfig::default(), vec![], Table::new(), game);
        assert_eq!(view.game.keys().collect::<Vec<_>>(), ["video.shader"]);
        assert_eq!(view.input.game.len(), 1);
    }

    #[test]
    fn single_key_writes_are_checked() {
        assert_eq!(settable_key_refusal("video.shader", true), None);
        assert_eq!(settable_key_refusal("achievements.enabled", false), None);
        assert!(settable_key_refusal("achievements.enabled", true).is_some());
        assert!(settable_key_refusal("cloud.provider", true).is_some());
        assert_eq!(settable_key_refusal("cloud.sync_states", false), None);
        assert_eq!(settable_key_refusal("cloud.sync_on_start", false), None);
        for key in ["cloud.provider", "cloud.local_folder"] {
            assert!(settable_key_refusal(key, false).is_some(), "{key}");
        }
        assert!(settable_key_refusal("video.future", false).is_some());
        assert_eq!(reset_controls_edits().len(), 64);
    }
}
