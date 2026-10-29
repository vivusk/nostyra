//! Layered gameplay config shared by the runner and the launcher.
//!
//! TOML files under `<data>/config/` are read lowest first — `global.toml`,
//! `system/<system>.toml`, `core/<core id>.toml`, `game/<system>/<rom key> [<ID>].toml` —
//! deep-merged (tables merge, scalars/arrays replace) and turned into a typed
//! [`ResolvedConfig`]. A missing file is an empty layer; an invalid value only
//! drops that key (lower layers / defaults show through); a file that is not TOML
//! is skipped with a warning. CLI flags (runner) win over every layer.
//!
//! Input bindings (`input_keyboard`, `input_gamepad`, `hotkeys_keyboard`,
//! `hotkeys_gamepad`) are two-level like every other section; their built-in
//! defaults ([`defaults_table`]) are merged as the lowest layer, and cross-key
//! conflicts are resolved after merging ([`resolve_input_conflicts`]). This
//! crate owns every action, hotkey and input name ([`PAD_ACTIONS`], …).
//!
//! Files are app-managed but hand-editable: writes keep unknown keys, but not
//! comments (plain `toml`, no format-preserving editor).

#![deny(unsafe_code)]

mod achievements_config;
mod atomic_file_write;
mod config_layer_paths;
mod config_layer_reader;
mod config_layer_writer;
mod core_layer_id;
mod input_binding_defaults;
mod input_binding_names;
mod input_binding_schema;
mod input_conflicts;
mod known_key_checks;
mod known_values_view;
mod resolved_config_diff;
mod resolved_config_schema;
mod resume_config_schema;
mod staged_dir_replace;
mod toml_table_deep_merge;

use std::path::PathBuf;

pub use achievements_config::{AchievementsConfig, ACHIEVEMENTS_SECTION};
pub use atomic_file_write::write_atomically;
pub use config_layer_paths::{global_config_path, ConfigLayerPaths};
pub use config_layer_reader::{
    resolve_config, resolve_config_with_warnings, resolve_layers_with_warnings,
};
pub use config_layer_writer::{set_layer_value, set_layer_values};
pub use core_layer_id::{bundled_preset_rel_path, core_layer_id, BUNDLED_PRESETS};
pub use input_binding_defaults::{
    DefaultBinds, DEFAULT_HOTKEYS_GAMEPAD, DEFAULT_HOTKEYS_KEYBOARD, DEFAULT_INPUT_GAMEPAD,
    DEFAULT_INPUT_KEYBOARD,
};
pub use input_binding_names::{
    gamepad_input_label, GAMEPAD_INPUT_LABELS, GAMEPAD_INPUT_NAMES, HOTKEYS,
    HOTKEYS_GAMEPAD_SECTION, HOTKEYS_KEYBOARD_SECTION, INPUT_GAMEPAD_SECTION,
    INPUT_KEYBOARD_SECTION, INPUT_KEY_PREFIXES, KEYBOARD_KEY_NAMES, MAX_BINDS_PER_ACTION,
    PAD_ACTIONS, RESERVED_GAMEPAD_INPUTS, RESERVED_KEYBOARD_KEYS, RESERVED_PAD_HOTKEY_INPUTS,
};
pub use input_binding_schema::{defaults_table, Binds, InputConfig};
pub use input_conflicts::resolve_input_conflicts;
pub use known_key_checks::MENU_OPTION_KEYS;
pub use known_values_view::{layer_known_values, resolved_known_values};
pub use resolved_config_diff::{changed_keys, known_keys};
pub use resolved_config_schema::{
    is_valid_shader, LatencyConfig, ResolvedConfig, RewindConfig, VideoConfig, VideoScaling,
    MAX_RUN_AHEAD_FRAMES, REWIND_BUFFER_MB_RANGE, SHADER_NONE,
};
pub use resume_config_schema::{ResumeConfig, ResumeMode};
pub use staged_dir_replace::{remove_stale_staging, replace_dir, staging_dirs};
pub use toml::{Table, Value};

/// Why a config layer could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{} is not valid TOML: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("cannot serialise config: {0}")]
    Serialize(#[from] toml::ser::Error),
    /// Empty key segment, or a segment that crosses a non-table value.
    #[error("invalid config key `{0}`")]
    InvalidKey(String),
    /// A known key with a value its validation rejects (e.g. `run_ahead_frames = 7`).
    #[error("invalid value for `{key}`: {value}")]
    InvalidValue { key: String, value: String },
}
