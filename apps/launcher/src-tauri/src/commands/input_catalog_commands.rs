//! Controls editor commands: the binding catalog (every name, label, reserved
//! input and default straight from `config-layers`, so TS hand-keeps no list),
//! assigning an input with steal, and resetting a scope's controls. Single-key
//! edits (remove a chip, Clear, Use default) go through `set_gameplay_config_value`.

use std::path::Path;

use config_layers::{
    defaults_table, known_keys, set_layer_values, ResolvedConfig, Table, Value,
    GAMEPAD_INPUT_LABELS, HOTKEYS, KEYBOARD_KEY_NAMES, MAX_BINDS_PER_ACTION, PAD_ACTIONS,
    RESERVED_GAMEPAD_INPUTS, RESERVED_KEYBOARD_KEYS, RESERVED_PAD_HOTKEY_INPUTS,
};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::command_error::CommandResult;
use crate::commands::blocking::run_blocking;
use crate::commands::gameplay_config_commands::{
    scope_layer_and_config, scope_layer_path, ConfigScope,
};
use crate::commands::input_binding_assign::{plan_assign, InputDevice};
use crate::commands::input_config_view::is_input_key;

/// One pad input with its printed labels.
#[derive(Debug, Serialize)]
pub struct GamepadInputInfo {
    pub name: &'static str,
    pub xbox: &'static str,
    pub sony: &'static str,
}

/// Everything the Controls editor needs to list, label and validate binds.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputCatalog {
    pub pad_actions: &'static [&'static str],
    /// In priority / display order.
    pub hotkeys: &'static [&'static str],
    /// winit `KeyCode` names (= DOM `code` except `Meta*` → `Super*`), reserved included.
    pub keyboard_keys: &'static [&'static str],
    pub gamepad_inputs: Vec<GamepadInputInfo>,
    pub reserved_keys: &'static [&'static str],
    pub reserved_gamepad_inputs: &'static [&'static str],
    /// Bindable as game actions but not as pad hotkeys (Select+Start menu chord).
    pub reserved_pad_hotkey_inputs: &'static [&'static str],
    pub max_binds: usize,
    /// `section → action → inputs` built-in defaults ("Default" labels).
    pub defaults: Table,
}

/// Result of [`assign_binding`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignResult {
    /// Dotted keys the input was taken from (toast "Z removed from Y").
    pub removed_from: Vec<String>,
}

#[tauri::command]
pub fn get_input_catalog() -> InputCatalog {
    InputCatalog {
        pad_actions: &PAD_ACTIONS,
        hotkeys: &HOTKEYS,
        keyboard_keys: KEYBOARD_KEY_NAMES,
        gamepad_inputs: GAMEPAD_INPUT_LABELS
            .iter()
            .map(|(name, (xbox, sony))| GamepadInputInfo { name, xbox, sony })
            .collect(),
        reserved_keys: &RESERVED_KEYBOARD_KEYS,
        reserved_gamepad_inputs: &RESERVED_GAMEPAD_INPUTS,
        reserved_pad_hotkey_inputs: &RESERVED_PAD_HOTKEY_INPUTS,
        max_binds: MAX_BINDS_PER_ACTION,
        defaults: defaults_table(),
    }
}

/// Binds `input` to `target` (dotted key) in `scope`, stealing it from every other
/// action / hotkey of `device` in one layer write. `invalid` for unknown targets,
/// unknown / reserved inputs and a full target.
#[tauri::command]
pub async fn assign_binding(
    app: AppHandle,
    scope: ConfigScope,
    device: InputDevice,
    target: String,
    input: String,
) -> CommandResult<AssignResult> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        let (layer, config) = scope_layer_and_config(&state, &scope)?;
        let removed_from = assign_in_layer(&layer, &config, device, &target, &input)?;
        Ok(AssignResult { removed_from })
    })
    .await
}

/// Removes every binding key from `scope`'s layer (gameplay and resume keys stay).
#[tauri::command]
pub async fn reset_controls(app: AppHandle, scope: ConfigScope) -> CommandResult<()> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        reset_controls_in(&scope_layer_path(&state, &scope)?)
    })
    .await
}

/// Plans against `config` (the scope's resolved view) and writes the plan to `layer`.
fn assign_in_layer(
    layer: &Path,
    config: &ResolvedConfig,
    device: InputDevice,
    target: &str,
    input: &str,
) -> CommandResult<Vec<String>> {
    let plan = plan_assign(&config.input, device, target, input)?;
    set_layer_values(layer, &plan.writes)?;
    tracing::info!(layer = %layer.display(), target, input, removed = ?plan.removed_from,
        "binding assigned");
    Ok(plan.removed_from)
}

fn reset_controls_in(layer: &Path) -> CommandResult<()> {
    let entries: Vec<(String, Option<Value>)> = known_keys()
        .filter(|key| is_input_key(key))
        .map(|key| (key, None))
        .collect();
    set_layer_values(layer, &entries)?;
    tracing::info!(layer = %layer.display(), "controls reset");
    Ok(())
}

#[cfg(test)]
#[path = "input_catalog_commands_tests.rs"]
mod tests;
