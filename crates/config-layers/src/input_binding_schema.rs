//! The four binding sections (`input_keyboard`, `input_gamepad`,
//! `hotkeys_keyboard`, `hotkeys_gamepad`): typed [`InputConfig`], per-key value
//! checks and the built-in defaults as the lowest merge layer.
//!
//! Each key is `action = ["Input", …]` (0–2 names): `[]` = explicitly unbound,
//! key absent = inherit. Arrays replace on merge, so a layer overriding one
//! action leaves every other action inherited. Defaults are a merge layer rather
//! than a serde default because a serde default only fires when a whole section
//! is missing and could not fill the untouched actions of a partial section.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use toml::{Table, Value};

use crate::input_binding_defaults::{
    DefaultBinds, DEFAULT_HOTKEYS_GAMEPAD, DEFAULT_HOTKEYS_KEYBOARD, DEFAULT_INPUT_GAMEPAD,
    DEFAULT_INPUT_KEYBOARD,
};
use crate::input_binding_names::{
    GAMEPAD_INPUT_NAMES, HOTKEYS, HOTKEYS_GAMEPAD_SECTION, HOTKEYS_KEYBOARD_SECTION,
    INPUT_GAMEPAD_SECTION, INPUT_KEYBOARD_SECTION, KEYBOARD_KEY_NAMES, MAX_BINDS_PER_ACTION,
    PAD_ACTIONS, RESERVED_GAMEPAD_INPUTS, RESERVED_KEYBOARD_KEYS, RESERVED_PAD_HOTKEY_INPUTS,
};
use crate::known_key_checks::KeyCheck;

/// Action or hotkey name → bound input names.
pub type Binds = BTreeMap<String, Vec<String>>;

/// Resolved binds of every section, always complete (every action and hotkey
/// present) thanks to the defaults layer. Serialised flat (`input_keyboard` …
/// at the top level of [`crate::ResolvedConfig`]) like the TOML files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputConfig {
    #[serde(rename = "input_keyboard")]
    pub keyboard: Binds,
    #[serde(rename = "input_gamepad")]
    pub gamepad: Binds,
    pub hotkeys_keyboard: Binds,
    pub hotkeys_gamepad: Binds,
}

/// Built from the same default slices as [`defaults_table`], so
/// `ResolvedConfig::default()` equals resolving no layers.
impl Default for InputConfig {
    fn default() -> Self {
        let binds = |defaults: &DefaultBinds| -> Binds {
            defaults
                .iter()
                .map(|(action, inputs)| {
                    let inputs = inputs.iter().map(|s| (*s).to_string()).collect();
                    ((*action).to_string(), inputs)
                })
                .collect()
        };
        Self {
            keyboard: binds(DEFAULT_INPUT_KEYBOARD),
            gamepad: binds(DEFAULT_INPUT_GAMEPAD),
            hotkeys_keyboard: binds(DEFAULT_HOTKEYS_KEYBOARD),
            hotkeys_gamepad: binds(DEFAULT_HOTKEYS_GAMEPAD),
        }
    }
}

/// One binding section: name, its keys, the value check, its defaults.
type BindingSection = (
    &'static str,
    &'static [&'static str],
    fn(&Value) -> bool,
    &'static DefaultBinds,
);

/// Every binding section, in known-key order.
const BINDING_SECTIONS: [BindingSection; 4] = [
    (
        INPUT_KEYBOARD_SECTION,
        &PAD_ACTIONS,
        keyboard_binds_valid,
        DEFAULT_INPUT_KEYBOARD,
    ),
    (
        INPUT_GAMEPAD_SECTION,
        &PAD_ACTIONS,
        gamepad_binds_valid,
        DEFAULT_INPUT_GAMEPAD,
    ),
    (
        HOTKEYS_KEYBOARD_SECTION,
        &HOTKEYS,
        keyboard_binds_valid,
        DEFAULT_HOTKEYS_KEYBOARD,
    ),
    (
        HOTKEYS_GAMEPAD_SECTION,
        &HOTKEYS,
        pad_hotkey_binds_valid,
        DEFAULT_HOTKEYS_GAMEPAD,
    ),
];

/// The defaults as a layer-shaped table, merged under the global layer.
pub fn defaults_table() -> Table {
    BINDING_SECTIONS
        .iter()
        .map(|(section, _, _, defaults)| {
            let binds: Table = defaults
                .iter()
                .map(|(action, inputs)| {
                    let inputs = inputs.iter().map(|s| Value::from(*s)).collect();
                    ((*action).to_string(), Value::Array(inputs))
                })
                .collect();
            ((*section).to_string(), Value::Table(binds))
        })
        .collect()
}

/// Known-key checks of every `section.action` of the binding sections.
pub(crate) fn binding_key_checks() -> impl Iterator<Item = KeyCheck> {
    BINDING_SECTIONS
        .iter()
        .flat_map(|(section, keys, valid, _)| keys.iter().map(move |key| (*section, *key, *valid)))
}

/// Drops keys of a binding section that name no action / hotkey (typos), with
/// a warning naming the layer, like an invalid value.
pub(crate) fn drop_unknown_binding_keys(layer: &mut Table, source: &Path) {
    for (section, keys, _, _) in &BINDING_SECTIONS {
        let Some(Value::Table(table)) = layer.get_mut(*section) else {
            continue;
        };
        table.retain(|key, _| {
            let known = keys.contains(&key);
            if !known {
                tracing::warn!(layer = %source.display(), key = %format!("{section}.{key}"),
                    "unknown binding key ignored");
            }
            known
        });
    }
}

fn keyboard_binds_valid(value: &Value) -> bool {
    binds_valid(value, KEYBOARD_KEY_NAMES, &RESERVED_KEYBOARD_KEYS)
}

fn gamepad_binds_valid(value: &Value) -> bool {
    binds_valid(value, &GAMEPAD_INPUT_NAMES, &RESERVED_GAMEPAD_INPUTS)
}

fn pad_hotkey_binds_valid(value: &Value) -> bool {
    gamepad_binds_valid(value)
        && !value.as_array().into_iter().flatten().any(|v| {
            v.as_str()
                .is_some_and(|s| RESERVED_PAD_HOTKEY_INPUTS.contains(&s))
        })
}

/// An array of at most [`MAX_BINDS_PER_ACTION`] distinct names from `names`,
/// none of them `reserved`.
fn binds_valid(value: &Value, names: &[&str], reserved: &[&str]) -> bool {
    let Some(items) = value.as_array() else {
        return false;
    };
    let mut seen = HashSet::new();
    items.len() <= MAX_BINDS_PER_ACTION
        && items.iter().all(|item| {
            item.as_str().is_some_and(|name| {
                names.contains(&name) && !reserved.contains(&name) && seen.insert(name)
            })
        })
}

#[cfg(test)]
#[path = "input_binding_schema_tests.rs"]
mod tests;
