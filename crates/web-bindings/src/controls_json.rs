//! The launcher's gameplay settings and Controls editor on layer texts (web
//! edition): the same view, single-key checks, assign-with-steal and "Reset
//! controls" the desktop commands apply to files (`config-layers`). Layers are the
//! texts of global, system, core, game (lowest first, `None` = missing file); a
//! scope without a system is the global scope (only the global layer counts, as on
//! the desktop's no-game Settings page), with a system it is that game's layer.

use config_layers::{
    gameplay_config_view, input_catalog, known_values_of_layer, parse_layer_text, plan_assign,
    reset_controls_edits, settable_key_refusal, GameplayConfigView, InputCatalog, InputDevice,
    Table,
};
use serde::Serialize;

use crate::config_json::{edit_text, resolve_stack, set_values, KeyEdit};

/// Index of the game layer in a layer stack.
const GAME_LAYER: usize = 3;

/// The settings view plus the layers skipped as not TOML (the page logs them).
#[derive(Debug, Serialize)]
pub struct SettingsView {
    pub view: GameplayConfigView,
    pub skipped: Vec<String>,
}

/// Result of an assignment: the edited layer's new text and the dotted keys the
/// input was taken from (toast "Z removed from Y").
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignOutcome {
    pub text: String,
    pub removed_from: Vec<String>,
}

pub fn catalog() -> InputCatalog {
    input_catalog()
}

/// `get_gameplay_config` of a scope.
pub fn settings_view(system_id: Option<&str>, layers: &[Option<String>]) -> SettingsView {
    let layers = scope_layers(system_id, layers);
    let stack = resolve_stack(system_id, &layers);
    let global = known_values(layers[0].as_deref());
    let game = known_values(layers[GAME_LAYER].as_deref());
    SettingsView {
        view: gameplay_config_view(&stack.config, stack.conflicts, global, game),
        skipped: stack.skipped,
    }
}

/// `set_gameplay_config_value`: new text of the scope's layer (`text`) with `key`
/// set or cleared; unknown keys and global-only keys in a game are refused.
pub fn set_known_value(
    text: Option<&str>,
    edit: KeyEdit,
    game_scope: bool,
) -> Result<String, String> {
    if let Some(reason) = settable_key_refusal(&edit.key, game_scope) {
        return Err(reason);
    }
    set_values(text, &[edit])
}

/// `assign_binding`: binds `input` to `target` in the scope's layer, stealing it from
/// the device's other actions / hotkeys as the scope resolves them.
pub fn assign(
    system_id: Option<&str>,
    layers: &[Option<String>],
    device: &str,
    target: &str,
    input: &str,
) -> Result<AssignOutcome, String> {
    let device: InputDevice = serde_json::from_value(serde_json::Value::from(device))
        .map_err(|_| format!("Unknown input device `{device}`."))?;
    let layers = scope_layers(system_id, layers);
    let stack = resolve_stack(system_id, &layers);
    let plan = plan_assign(&stack.config.input, device, target, input).map_err(|err| err.0)?;
    let edited = if system_id.is_some() { GAME_LAYER } else { 0 };
    let text = edit_text(layers[edited].as_deref(), &plan.writes)?;
    Ok(AssignOutcome {
        text,
        removed_from: plan.removed_from,
    })
}

/// `reset_controls`: the layer's text without any binding key.
pub fn reset_controls(text: Option<&str>) -> Result<String, String> {
    edit_text(text, &reset_controls_edits())
}

/// The four layers a scope reads: everything for a game, the global layer alone
/// without one.
fn scope_layers(system_id: Option<&str>, layers: &[Option<String>]) -> [Option<String>; 4] {
    let layer = |index: usize| layers.get(index).cloned().flatten();
    match system_id {
        Some(_) => [layer(0), layer(1), layer(2), layer(GAME_LAYER)],
        None => [layer(0), None, None, None],
    }
}

/// Known values one layer sets; a layer that is not TOML shows as empty (as on the
/// desktop, where a later write reports the problem).
fn known_values(text: Option<&str>) -> Table {
    let source = std::path::Path::new("config/layer.toml");
    text.and_then(|text| parse_layer_text(text, source).ok())
        .map(|table| known_values_of_layer(table, source))
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "controls_json_tests.rs"]
mod tests;
