//! Reads layer files and resolves the effective config for one game. The merge
//! itself works on already-parsed tables ([`resolve_layer_tables_with_warnings`],
//! pure); the file readers (feature `fs`) only load the text.

use std::path::Path;

use toml::Table;

use crate::achievements_config::strip_global_only;
use crate::cloud_config::strip_cloud_section;
use crate::input_binding_schema::defaults_table;
use crate::input_conflicts::resolve_input_conflicts;
use crate::known_key_checks::drop_invalid_keys;
use crate::netplay_config::strip_netplay_section;
use crate::resolved_config_schema::{to_resolved, ResolvedConfig};
use crate::toml_table_deep_merge::deep_merge;
use crate::ConfigError;

/// Parses the text of one layer; `source` names the layer in errors and warnings.
pub fn parse_layer_text(text: &str, source: &Path) -> Result<Table, ConfigError> {
    toml::from_str(text).map_err(|err| ConfigError::Parse {
        path: source.to_path_buf(),
        source: err,
    })
}

/// Merges parsed layers (lowest first, the global layer first; `None` = missing
/// layer) over the built-in defaults: global-only sections (`[achievements]`, `[cloud]`, `[netplay]`) come
/// from the first layer alone, invalid values are dropped (logged via `tracing`),
/// binding conflicts are resolved and returned as warnings. `system_actions` = the
/// game system's pad actions (`None` for no-game views).
pub fn resolve_layer_tables_with_warnings(
    layers: Vec<(&Path, Option<Table>)>,
    system_actions: Option<&[&str]>,
) -> (ResolvedConfig, Vec<String>) {
    let mut merged = defaults_table();
    for (index, (source, layer)) in layers.into_iter().enumerate() {
        let Some(mut layer) = layer else { continue };
        if index > 0 {
            strip_global_only(&mut layer, source);
            strip_cloud_section(&mut layer, source);
            strip_netplay_section(&mut layer, source);
        }
        drop_invalid_keys(&mut layer, source);
        deep_merge(&mut merged, layer);
    }
    let mut config = to_resolved(merged);
    let warnings = resolve_input_conflicts(&mut config.input, system_actions);
    (config, warnings)
}

/// One layer file as a raw table; `Ok(None)` when the file does not exist.
#[cfg(feature = "fs")]
pub(crate) fn read_layer_table(path: &Path) -> Result<Option<Table>, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_layer_text(&text, path).map(Some),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ConfigError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Merges every layer (global → system → core → game) over the built-in
/// defaults. Never fails: unreadable or non-TOML layers are skipped and invalid
/// values dropped, each with a warning naming the file. Binding conflicts are
/// resolved and only logged (the runner needs no warning list). `system_actions`
/// = the game system's pad actions (`None` for no-game views), see
/// [`resolve_input_conflicts`].
#[cfg(feature = "fs")]
pub fn resolve_config(
    paths: &crate::ConfigLayerPaths,
    system_actions: Option<&[&str]>,
) -> ResolvedConfig {
    let (config, warnings) = resolve_config_with_warnings(paths, system_actions);
    for warning in warnings {
        tracing::warn!("input binding conflict: {warning}");
    }
    config
}

/// [`resolve_config`] plus the binding-conflict warnings (launcher shows them).
#[cfg(feature = "fs")]
pub fn resolve_config_with_warnings(
    paths: &crate::ConfigLayerPaths,
    system_actions: Option<&[&str]>,
) -> (ResolvedConfig, Vec<String>) {
    resolve_layers_with_warnings(&paths.lowest_first(), system_actions)
}

/// Resolves any layer stack (lowest first, the global layer first) over the
/// defaults layer, e.g. only `[global]` for the launcher's no-game Settings view.
/// Unreadable or non-TOML layers are skipped with a warning.
#[cfg(feature = "fs")]
pub fn resolve_layers_with_warnings(
    layers: &[&Path],
    system_actions: Option<&[&str]>,
) -> (ResolvedConfig, Vec<String>) {
    let tables = layers
        .iter()
        .map(|path| {
            let table = read_layer_table(path).unwrap_or_else(|err| {
                tracing::warn!("config layer skipped: {err}");
                None
            });
            (*path, table)
        })
        .collect();
    resolve_layer_tables_with_warnings(tables, system_actions)
}
