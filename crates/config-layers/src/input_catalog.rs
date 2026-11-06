//! The Controls editor's catalog: every action, hotkey and input name, label,
//! reserved input and default, straight from this crate, so neither the launcher
//! (desktop `get_input_catalog`) nor the web edition hand-keeps a list.

use serde::Serialize;
use toml::Table;

use crate::input_binding_names::{
    GAMEPAD_INPUT_LABELS, HOTKEYS, KEYBOARD_KEY_NAMES, MAX_BINDS_PER_ACTION, PAD_ACTIONS,
    RESERVED_GAMEPAD_INPUTS, RESERVED_KEYBOARD_KEYS, RESERVED_PAD_HOTKEY_INPUTS,
};
use crate::input_binding_schema::defaults_table;

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

/// The catalog (JSON field names = the launcher's `InputCatalog` type).
pub fn input_catalog() -> InputCatalog {
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
