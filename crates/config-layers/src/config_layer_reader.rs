//! Reads layer files and resolves the effective config for one game.

use std::path::Path;

use toml::Table;

use crate::achievements_config::strip_global_only;
use crate::config_layer_paths::ConfigLayerPaths;
use crate::input_binding_schema::defaults_table;
use crate::input_conflicts::resolve_input_conflicts;
use crate::known_key_checks::drop_invalid_keys;
use crate::resolved_config_schema::{to_resolved, ResolvedConfig};
use crate::toml_table_deep_merge::deep_merge;
use crate::ConfigError;

/// One layer file as a raw table; `Ok(None)` when the file does not exist.
pub(crate) fn read_layer_table(path: &Path) -> Result<Option<Table>, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    toml::from_str(&text)
        .map(Some)
        .map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

/// Merges every layer (global → system → core → game) over the built-in
/// defaults. Never fails: unreadable or non-TOML layers are skipped and invalid
/// values dropped, each with a warning naming the file. Binding conflicts are
/// resolved and only logged (the runner needs no warning list). `system_actions`
/// = the game system's pad actions (`None` for no-game views), see
/// [`resolve_input_conflicts`].
pub fn resolve_config(paths: &ConfigLayerPaths, system_actions: Option<&[&str]>) -> ResolvedConfig {
    let (config, warnings) = resolve_config_with_warnings(paths, system_actions);
    for warning in warnings {
        tracing::warn!("input binding conflict: {warning}");
    }
    config
}

/// [`resolve_config`] plus the binding-conflict warnings (launcher shows them).
pub fn resolve_config_with_warnings(
    paths: &ConfigLayerPaths,
    system_actions: Option<&[&str]>,
) -> (ResolvedConfig, Vec<String>) {
    resolve_layers_with_warnings(&paths.lowest_first(), system_actions)
}

/// Resolves any layer stack (lowest first, the global layer first) over the
/// defaults layer, e.g. only `[global]` for the launcher's no-game Settings view.
/// Global-only sections (`[achievements]`) are read from the first layer alone. Returns the
/// binding-conflict warnings; per-key invalid values are logged via `tracing`.
pub fn resolve_layers_with_warnings(
    layers: &[&Path],
    system_actions: Option<&[&str]>,
) -> (ResolvedConfig, Vec<String>) {
    let mut merged = defaults_table();
    for (index, path) in layers.iter().enumerate() {
        match read_layer_table(path) {
            Ok(Some(mut layer)) => {
                if index > 0 {
                    strip_global_only(&mut layer, path);
                }
                drop_invalid_keys(&mut layer, path);
                deep_merge(&mut merged, layer);
            }
            Ok(None) => {}
            Err(err) => tracing::warn!("config layer skipped: {err}"),
        }
    }
    let mut config = to_resolved(merged);
    let warnings = resolve_input_conflicts(&mut config.input, system_actions);
    (config, warnings)
}
