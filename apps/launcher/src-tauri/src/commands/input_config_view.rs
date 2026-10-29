//! Splits the flat config views into the gameplay part (menu options + `resume.*`,
//! what the M3 forms and Detail "Reset all to defaults" act on) and the controls
//! part (the four binding sections), so a gameplay reset can never touch a binding.

use config_layers::{
    resolve_layers_with_warnings, resolved_known_values, ResolvedConfig, Table, INPUT_KEY_PREFIXES,
};
use serde::Serialize;

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

/// Whether a dotted key belongs to one of the binding sections.
pub fn is_input_key(key: &str) -> bool {
    INPUT_KEY_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
}

/// Whether a dotted key is global-layer only (`[achievements]`).
pub fn is_global_only_key(key: &str) -> bool {
    key.starts_with("achievements.")
}

/// `(gameplay keys, binding keys)` of one flat view.
pub fn split_input_keys(table: Table) -> (Table, Table) {
    table.into_iter().partition(|(key, _)| !is_input_key(key))
}

/// The no-game view: only the global layer, resolved over the built-in defaults
/// exactly like a game view (so global-layer conflicts are resolved and reported),
/// without a system (hotkeys win; conflicts between defaults stay silent).
pub fn resolve_global_only(global: &std::path::Path) -> (ResolvedConfig, Vec<String>) {
    resolve_layers_with_warnings(&[global], None)
}

/// A game layer's `(gameplay, binding)` keys without the global-only ones, which the
/// runner ignores there (so the per-game form never offers or resets them).
pub fn split_game_layer(mut table: Table) -> (Table, Table) {
    table.retain(|key, _| !is_global_only_key(key));
    split_input_keys(table)
}

/// Flat resolved values split into `(gameplay, binding)` keys.
pub fn split_resolved(config: &ResolvedConfig) -> (Table, Table) {
    split_input_keys(resolved_known_values(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_view_splits_controls_from_gameplay() {
        let (gameplay, input) = split_resolved(&ResolvedConfig::default());
        assert!(gameplay.keys().all(|key| !is_input_key(key)));
        assert!(gameplay.contains_key("video.shader") && gameplay.contains_key("resume.mode"));
        assert_eq!(
            gameplay.len(),
            12,
            "5 menu options + 2 resume + 5 achievements keys"
        );
        assert_eq!(input.len(), 48 + 18, "2 × 24 actions + 2 × 9 hotkeys");
        assert!(input.keys().all(|key| is_input_key(key)));
        assert!(!is_input_key("input.keyboard") && is_input_key("hotkeys_gamepad.save_state"));
    }
}
