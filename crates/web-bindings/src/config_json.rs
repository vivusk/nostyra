//! Config layers on text (the web reads/writes the TOML files of the shared Data
//! folder itself): resolve a layer stack, one layer's known values, edit a layer.
//! Same parse, merge, validation and conflict rules as the desktop (config-layers).

use std::path::Path;

use config_layers::{
    edit_layer_table, known_values_of_layer, parse_layer_text, resolve_layer_tables_with_warnings,
    resolved_known_values, ResolvedConfig, Table, Value,
};
use serde::Serialize;

/// Labels for warnings, lowest layer first (`global`, `system`, `core`, `game`).
const LAYER_LABELS: [&str; 4] = [
    "config/global.toml",
    "config/system.toml",
    "config/core.toml",
    "config/game.toml",
];

/// Effective known values (dotted keys) and binding-conflict warnings.
#[derive(Debug, Serialize)]
pub struct ResolvedView {
    pub values: Table,
    pub warnings: Vec<String>,
}

/// Resolves `layers` (text of global, system, core, game, lowest first; `None` =
/// missing file) for a game of `system_id` (`None`: no-game Settings view). A layer
/// that is not TOML is skipped with a warning, like the desktop.
pub fn resolve(system_id: Option<&str>, layers: &[Option<String>]) -> ResolvedView {
    let stack = resolve_stack(system_id, layers);
    let mut warnings = stack.skipped;
    warnings.extend(stack.conflicts);
    ResolvedView {
        values: resolved_known_values(&stack.config),
        warnings,
    }
}

/// A resolved layer stack: the typed config, binding-conflict warnings (Controls
/// notice) and the layers skipped as not TOML (logged only).
pub struct ResolvedStack {
    pub config: ResolvedConfig,
    pub conflicts: Vec<String>,
    pub skipped: Vec<String>,
}

/// [`resolve`] keeping the typed config and the two kinds of warnings apart.
pub fn resolve_stack(system_id: Option<&str>, layers: &[Option<String>]) -> ResolvedStack {
    let mut skipped = Vec::new();
    let tables = layers
        .iter()
        .zip(LAYER_LABELS)
        .map(|(text, label)| {
            let source = Path::new(label);
            let table = text.as_deref().and_then(|text| {
                parse_layer_text(text, source)
                    .map_err(|err| skipped.push(format!("config layer skipped: {err}")))
                    .ok()
            });
            (source, table)
        })
        .collect();
    let actions = system_id
        .and_then(system_catalog::system)
        .map(|spec| spec.pad_actions);
    let (config, conflicts) = resolve_layer_tables_with_warnings(tables, actions);
    ResolvedStack {
        config,
        conflicts,
        skipped,
    }
}

/// The valid known keys one layer's text sets (empty for `None`).
pub fn layer_values(text: Option<&str>) -> Result<Table, String> {
    let Some(text) = text else {
        return Ok(Table::new());
    };
    let source = Path::new("config/layer.toml");
    let table = parse_layer_text(text, source).map_err(|err| err.to_string())?;
    Ok(known_values_of_layer(table, source))
}

/// One key edit: `value` `None` removes the key.
#[derive(Debug, serde::Deserialize)]
pub struct KeyEdit {
    pub key: String,
    pub value: Option<serde_json::Value>,
}

/// Applies `edits` to a layer's text (`None` = new file) and returns the new text;
/// all or nothing (an invalid key or value fails the call, text unchanged).
pub fn set_values(text: Option<&str>, edits: &[KeyEdit]) -> Result<String, String> {
    let entries = edits
        .iter()
        .map(|edit| {
            let value = edit
                .value
                .clone()
                .map(Value::try_from)
                .transpose()
                .map_err(|err| format!("{}: {err}", edit.key))?;
            Ok((edit.key.as_str(), value))
        })
        .collect::<Result<Vec<_>, String>>()?;
    edit_text(text, &entries)
}

/// Applies typed `entries` to a layer's text (`None` = new file); all or nothing.
pub fn edit_text<K: AsRef<str>>(
    text: Option<&str>,
    entries: &[(K, Option<Value>)],
) -> Result<String, String> {
    let source = Path::new("config/layer.toml");
    let mut table = match text {
        Some(text) => parse_layer_text(text, source).map_err(|err| err.to_string())?,
        None => Table::new(),
    };
    edit_layer_table(&mut table, entries).map_err(|err| err.to_string())?;
    config_layers::layer_table_text(&table).map_err(|err| err.to_string())
}

#[cfg(test)]
#[path = "config_json_tests.rs"]
mod tests;
