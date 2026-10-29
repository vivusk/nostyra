//! Bindings built once at start from the resolved config (`config_layers::InputConfig`),
//! replacing the hard-coded M1–M3 tables: keyboard keys / pad inputs → SNES
//! button bitmasks and stick directions, and → runner hotkeys. Defaults live in
//! `config-layers` (the lowest merge layer), so with no user config this equals
//! the old layout.
//!
//! Esc → pause menu is added on top of the config (fixed, config-layers refuses
//! to bind it), so the menu is always reachable. Guide and the Select+Start
//! chord are fixed in the runner's pause-menu input and never bindings.

use std::collections::HashMap;

use config_layers::{
    resolve_input_conflicts, Binds, InputConfig, HOTKEYS_GAMEPAD_SECTION, HOTKEYS_KEYBOARD_SECTION,
    INPUT_GAMEPAD_SECTION, INPUT_KEYBOARD_SECTION,
};
use winit::keyboard::KeyCode;

use crate::game_action_bits::{action_bits, buttons, sticks};
use crate::gamepad_input_names::GamepadInput;
use crate::key_names::key_code_from_name;
use crate::runner_hotkeys::RunnerHotkey;

/// Every input → what it does. Small vectors for pads: at most a few dozen
/// entries, scanned per pad per frame without allocating.
#[derive(Clone, Debug, Default)]
pub struct InputBindings {
    /// Key → game action bits on port 1 (one key may press several buttons).
    keyboard: HashMap<KeyCode, u32>,
    /// Pad input → game action bits (every pad, its own port).
    gamepad: Vec<(GamepadInput, u32)>,
    keyboard_hotkeys: HashMap<KeyCode, RunnerHotkey>,
    gamepad_hotkeys: Vec<(GamepadInput, RunnerHotkey)>,
}

impl InputBindings {
    /// Builds the bindings; returns warnings for names this crate cannot convert
    /// (config-layers already dropped unknown names, so normally none) and for
    /// conflicts (re-resolved here with the same rules as config-layers, given the
    /// running system's pad actions; `None` = no system context).
    pub fn from_config(
        config: &InputConfig,
        system_actions: Option<&[&str]>,
    ) -> (Self, Vec<String>) {
        let mut config = config.clone();
        let mut warnings = resolve_input_conflicts(&mut config, system_actions);
        let mut bindings = Self::default();
        let keys = convert(
            (INPUT_KEYBOARD_SECTION, &config.keyboard),
            action_bits,
            key_code_from_name,
            &mut warnings,
        );
        for (key, mask) in keys {
            *bindings.keyboard.entry(key).or_default() |= mask;
        }
        let pads = convert(
            (INPUT_GAMEPAD_SECTION, &config.gamepad),
            action_bits,
            GamepadInput::from_name,
            &mut warnings,
        );
        for (input, mask) in pads {
            match bindings
                .gamepad
                .iter_mut()
                .find(|(bound, _)| *bound == input)
            {
                Some((_, bits)) => *bits |= mask,
                None => bindings.gamepad.push((input, mask)),
            }
        }
        let key_hotkeys = convert(
            (HOTKEYS_KEYBOARD_SECTION, &config.hotkeys_keyboard),
            RunnerHotkey::from_name,
            key_code_from_name,
            &mut warnings,
        );
        for (key, hotkey) in key_hotkeys {
            bindings.keyboard_hotkeys.entry(key).or_insert(hotkey);
        }
        bindings
            .keyboard_hotkeys
            .insert(KeyCode::Escape, RunnerHotkey::PauseMenu);
        let pad_hotkeys = convert(
            (HOTKEYS_GAMEPAD_SECTION, &config.hotkeys_gamepad),
            RunnerHotkey::from_name,
            GamepadInput::from_name,
            &mut warnings,
        );
        for (input, hotkey) in pad_hotkeys {
            if bindings.pad_hotkey(input).is_none() {
                bindings.gamepad_hotkeys.push((input, hotkey));
            }
        }
        (bindings, warnings)
    }

    /// SNES buttons (port 1) a key presses; 0 = none.
    pub fn key_mask(&self, key: KeyCode) -> u16 {
        buttons(self.key_bits(key))
    }

    /// Stick directions (port 1) a key holds; 0 = none.
    pub fn key_sticks(&self, key: KeyCode) -> u8 {
        sticks(self.key_bits(key))
    }

    /// Whether a key is bound to a game action (button or stick direction).
    pub fn is_game_key(&self, key: KeyCode) -> bool {
        self.key_bits(key) != 0
    }

    fn key_bits(&self, key: KeyCode) -> u32 {
        self.keyboard.get(&key).copied().unwrap_or(0)
    }

    /// Hotkey a key triggers (Esc → pause menu always).
    pub fn key_hotkey(&self, key: KeyCode) -> Option<RunnerHotkey> {
        self.keyboard_hotkeys.get(&key).copied()
    }

    /// SNES buttons pressed on one pad, given which of its inputs are `held`.
    /// Inputs bound to hotkeys are never game inputs (hotkey wins), so a pad
    /// hotkey can never leak into the core.
    pub fn pad_mask(&self, held: impl Fn(GamepadInput) -> bool) -> u16 {
        buttons(self.pad_bits(held))
    }

    /// Stick directions held on one pad by inputs bound to stick actions.
    pub fn pad_sticks(&self, held: impl Fn(GamepadInput) -> bool) -> u8 {
        sticks(self.pad_bits(held))
    }

    fn pad_bits(&self, held: impl Fn(GamepadInput) -> bool) -> u32 {
        self.gamepad
            .iter()
            .filter(|(input, _)| held(*input))
            .fold(0, |mask, (_, bits)| mask | bits)
    }

    /// Every pad input bound to game actions, with its action bits.
    pub(crate) fn pad_game_inputs(&self) -> impl Iterator<Item = (GamepadInput, u32)> + '_ {
        self.gamepad.iter().copied()
    }

    /// Hotkey a pad input triggers, if any.
    pub fn pad_hotkey(&self, input: GamepadInput) -> Option<RunnerHotkey> {
        self.gamepad_hotkeys
            .iter()
            .find(|(bound, _)| *bound == input)
            .map(|(_, hotkey)| *hotkey)
    }

    /// Whether any pad input bound to `hotkey` is `held` (hold hotkeys).
    pub fn pad_hotkey_held(
        &self,
        hotkey: RunnerHotkey,
        held: impl Fn(GamepadInput) -> bool,
    ) -> bool {
        self.gamepad_hotkeys
            .iter()
            .any(|(input, bound)| *bound == hotkey && held(*input))
    }
}

/// `(section, binds)` → `(input, target)` pairs; names either side cannot
/// convert are skipped with a warning (never a panic).
fn convert<I, T>(
    (section, binds): (&str, &Binds),
    target: impl Fn(&str) -> Option<T>,
    input: impl Fn(&str) -> Option<I>,
    warnings: &mut Vec<String>,
) -> Vec<(I, T)>
where
    T: Copy,
{
    let mut pairs = Vec::new();
    for (action, names) in binds {
        let Some(target) = target(action) else {
            warnings.push(format!("{section}.{action}: unknown action; ignored"));
            continue;
        };
        for name in names {
            match input(name) {
                Some(input) => pairs.push((input, target)),
                None => warnings.push(format!(
                    "{section}.{action}: unknown input {name:?}; ignored"
                )),
            }
        }
    }
    pairs
}

#[cfg(test)]
#[path = "input_bindings_tests.rs"]
mod tests;
