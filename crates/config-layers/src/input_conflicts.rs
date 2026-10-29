//! Cross-key binding conflicts, resolved after all layers merged. One owner for
//! runner and launcher: per device, an input on a hotkey is removed from game
//! actions (hotkey wins, so the menu and save keys stay reachable); an input on
//! two hotkeys stays on the first in [`HOTKEYS`] order; one input on two game
//! actions is allowed (it presses both).
//!
//! System-aware refinements (`system_actions` = the pad actions of the game's
//! system; `None` = no-game views):
//! - on the gamepad, the hold hotkeys (fast-forward, rewind) **yield** an input to a
//!   game action the system uses (PS1 L2/R2 on the triggers);
//! - game actions the system does not use never take an input from a hotkey and
//!   are dropped (no input reaches the core) silently (SNES `l2` / `r2`);
//! - a conflict between two built-in defaults is resolved silently; warnings are
//!   for bindings a user layer set.

use std::collections::HashMap;

use crate::input_binding_defaults::{
    DefaultBinds, DEFAULT_HOTKEYS_GAMEPAD, DEFAULT_HOTKEYS_KEYBOARD, DEFAULT_INPUT_GAMEPAD,
    DEFAULT_INPUT_KEYBOARD,
};
use crate::input_binding_names::{
    HOTKEYS, HOTKEYS_GAMEPAD_SECTION, HOTKEYS_KEYBOARD_SECTION, INPUT_GAMEPAD_SECTION,
    INPUT_KEYBOARD_SECTION, PAD_ACTIONS,
};
use crate::input_binding_schema::{Binds, InputConfig};

/// Gamepad hotkeys that give their input to a game action of the running system.
const HOLD_HOTKEYS: [&str; 2] = ["fast_forward_hold", "rewind_hold"];

/// One device's sections and built-in binds.
struct Device {
    hotkey_section: &'static str,
    action_section: &'static str,
    hotkey_defaults: &'static DefaultBinds,
    action_defaults: &'static DefaultBinds,
    /// Hold hotkeys yield to the system's game actions (gamepad only).
    holds_yield: bool,
}

const KEYBOARD: Device = Device {
    hotkey_section: HOTKEYS_KEYBOARD_SECTION,
    action_section: INPUT_KEYBOARD_SECTION,
    hotkey_defaults: DEFAULT_HOTKEYS_KEYBOARD,
    action_defaults: DEFAULT_INPUT_KEYBOARD,
    holds_yield: false,
};

const GAMEPAD: Device = Device {
    hotkey_section: HOTKEYS_GAMEPAD_SECTION,
    action_section: INPUT_GAMEPAD_SECTION,
    hotkey_defaults: DEFAULT_HOTKEYS_GAMEPAD,
    action_defaults: DEFAULT_INPUT_GAMEPAD,
    holds_yield: true,
};

/// Removes conflicting binds in place; returns one warning per removal a user
/// layer caused. `system_actions`: see the module docs.
pub fn resolve_input_conflicts(
    input: &mut InputConfig,
    system_actions: Option<&[&str]>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let keyboard = (&mut input.hotkeys_keyboard, &mut input.keyboard);
    resolve_device(keyboard, &KEYBOARD, system_actions, &mut warnings);
    let gamepad = (&mut input.hotkeys_gamepad, &mut input.gamepad);
    resolve_device(gamepad, &GAMEPAD, system_actions, &mut warnings);
    warnings
}

/// One device: hotkeys claim inputs in priority order, then each game action
/// either takes a hold hotkey's input (yield) or loses every claimed input.
fn resolve_device(
    (hotkeys, actions): (&mut Binds, &mut Binds),
    device: &Device,
    system_actions: Option<&[&str]>,
    warnings: &mut Vec<String>,
) {
    let hotkey_section = device.hotkey_section;
    let action_section = device.action_section;
    let default_hotkey = |key: &str, binds: &Binds| is_default(device.hotkey_defaults, key, binds);
    let default_action = |key: &str, binds: &Binds| is_default(device.action_defaults, key, binds);
    let mut claimed: HashMap<String, &'static str> = HashMap::new();
    for hotkey in HOTKEYS {
        let Some(inputs) = hotkeys.get_mut(hotkey) else {
            continue;
        };
        inputs.retain(|input| match claimed.get(input) {
            Some(first) => {
                warnings.push(format!(
                    "`{input}` is bound to both `{hotkey_section}.{first}` and \
                     `{hotkey_section}.{hotkey}`; kept on `{first}`"
                ));
                false
            }
            None => {
                claimed.insert(input.clone(), hotkey);
                true
            }
        });
    }
    let original_hotkeys = hotkeys.clone();
    for action in PAD_ACTIONS {
        let used = system_actions.is_none_or(|list| list.contains(&action));
        let action_default = default_action(action, actions);
        let Some(inputs) = actions.get_mut(action) else {
            continue;
        };
        if !used {
            // Not this system's button: never reaches the core, takes nothing from
            // a hotkey, and needs no warning.
            inputs.clear();
            continue;
        }
        let mut kept = Vec::with_capacity(inputs.len());
        for input in inputs.drain(..) {
            let Some(&hotkey) = claimed.get(&input) else {
                kept.push(input);
                continue;
            };
            let hotkey_default = default_hotkey(hotkey, &original_hotkeys);
            if device.holds_yield && system_actions.is_some() && HOLD_HOTKEYS.contains(&hotkey) {
                if let Some(hold) = hotkeys.get_mut(hotkey) {
                    hold.retain(|bound| *bound != input);
                }
                if !hotkey_default {
                    warnings.push(format!(
                        "`{input}` moved from `{hotkey_section}.{hotkey}` to game action \
                         `{action_section}.{action}`"
                    ));
                }
                kept.push(input);
                continue;
            }
            if !(hotkey_default && action_default) {
                warnings.push(format!(
                    "`{input}` is bound to hotkey `{hotkey_section}.{hotkey}`; removed from \
                     `{action_section}.{action}`"
                ));
            }
        }
        *inputs = kept;
    }
}

/// Whether `key`'s inputs in `binds` are exactly its built-in default.
fn is_default(defaults: &DefaultBinds, key: &str, binds: &Binds) -> bool {
    let current = binds.get(key).map_or(&[][..], Vec::as_slice);
    defaults
        .iter()
        .find(|(name, _)| *name == key)
        .is_some_and(|(_, inputs)| {
            inputs
                .iter()
                .copied()
                .eq(current.iter().map(String::as_str))
        })
}

#[cfg(test)]
#[path = "input_conflicts_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "input_conflicts_nds_tests.rs"]
mod nds_tests;

#[cfg(test)]
#[path = "input_conflicts_gc_wii_tests.rs"]
mod gc_wii_tests;
