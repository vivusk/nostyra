//! Controls editor commands: the binding catalog (every name, label, reserved
//! input and default straight from `config-layers`, so TS hand-keeps no list),
//! assigning an input with steal, and resetting a scope's controls. Single-key
//! edits (remove a chip, Clear, Use default) go through `set_gameplay_config_value`.

use std::path::Path;

use config_layers::{
    input_catalog, plan_assign, reset_controls_edits, set_layer_values, InputCatalog, InputDevice,
    ResolvedConfig,
};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::command_error::CommandResult;
use crate::commands::blocking::run_blocking;
use crate::commands::gameplay_config_commands::{
    scope_layer_and_config, scope_layer_path, ConfigScope,
};

/// Result of [`assign_binding`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignResult {
    /// Dotted keys the input was taken from (toast "Z removed from Y").
    pub removed_from: Vec<String>,
}

/// The shared catalog (`config_layers::input_catalog`, also used by the web edition).
#[tauri::command]
pub fn get_input_catalog() -> InputCatalog {
    input_catalog()
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
    set_layer_values(layer, &reset_controls_edits())?;
    tracing::info!(layer = %layer.display(), "controls reset");
    Ok(())
}

#[cfg(test)]
#[path = "input_catalog_commands_tests.rs"]
mod tests;
