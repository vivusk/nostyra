//! The no-game settings view's resolution (global layer over the built-in
//! defaults). The gameplay / controls split itself is shared with the web edition
//! (`config_layers::gameplay_config_view`).

use config_layers::{resolve_layers_with_warnings, ResolvedConfig};

/// The no-game view: only the global layer, resolved over the built-in defaults
/// exactly like a game view (so global-layer conflicts are resolved and reported),
/// without a system (hotkeys win; conflicts between defaults stay silent).
pub fn resolve_global_only(global: &std::path::Path) -> (ResolvedConfig, Vec<String>) {
    resolve_layers_with_warnings(&[global], None)
}
