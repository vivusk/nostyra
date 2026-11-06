//! wasm32 only: the gameplay settings view and Controls editor exports
//! (`controls_json`). Layer stacks travel as four optional texts (global, system,
//! core, game); `system` absent = the global scope. Same marshalling rules as
//! `wasm_exports`.

use wasm_bindgen::prelude::*;

use crate::config_json::KeyEdit;
use crate::controls_json;
use crate::wasm_exports::{js_err, to_json};

/// The Controls catalog (JSON, the launcher's `InputCatalog`).
#[wasm_bindgen]
pub fn input_catalog_json() -> Result<String, JsError> {
    to_json(&controls_json::catalog())
}

/// `{view, skipped}`: the launcher's `GameplayConfigView` of a scope.
#[wasm_bindgen]
pub fn gameplay_settings_view(
    system: Option<String>,
    global: Option<String>,
    system_layer: Option<String>,
    core: Option<String>,
    game: Option<String>,
) -> Result<String, JsError> {
    let layers = [global, system_layer, core, game];
    to_json(&controls_json::settings_view(system.as_deref(), &layers))
}

/// `{text, removedFrom}`: the scope layer's new text after binding `input` to
/// `target` with steal (refusals reject with the user-facing reason).
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn config_assign_binding(
    system: Option<String>,
    global: Option<String>,
    system_layer: Option<String>,
    core: Option<String>,
    game: Option<String>,
    device: &str,
    target: &str,
    input: &str,
) -> Result<String, JsError> {
    let layers = [global, system_layer, core, game];
    let outcome =
        controls_json::assign(system.as_deref(), &layers, device, target, input).map_err(js_err)?;
    to_json(&outcome)
}

/// New layer text with one known key set (`value_json` = JSON value) or cleared
/// (`"null"`); unknown keys and global-only keys in a game layer are refused.
#[wasm_bindgen]
pub fn config_set_known_value(
    text: Option<String>,
    key: &str,
    value_json: &str,
    game_scope: bool,
) -> Result<String, JsError> {
    let value: Option<serde_json::Value> = serde_json::from_str(value_json).map_err(js_err)?;
    let edit = KeyEdit {
        key: key.to_string(),
        value,
    };
    controls_json::set_known_value(text.as_deref(), edit, game_scope).map_err(js_err)
}

/// The layer's text without any binding key ("Reset controls").
#[wasm_bindgen]
pub fn config_reset_controls(text: Option<String>) -> Result<String, JsError> {
    controls_json::reset_controls(text.as_deref()).map_err(js_err)
}
