//! Keyboard half of the joypad input: the bound keys currently held. Masks are
//! derived from the held set at poll time (not toggled per event), so two keys
//! on one button, or one key on two buttons, release correctly.

use winit::keyboard::KeyCode;

use crate::input_bindings::InputBindings;
use crate::runner_hotkeys::RunnerHotkey;

/// Held game keys and held hold-hotkey keys (fast-forward / rewind).
#[derive(Debug, Default)]
pub(crate) struct KeyboardState {
    held: Vec<KeyCode>,
}

impl KeyboardState {
    /// Tracks a key press / release; true if the key is the joypad's (a game
    /// key or a hold hotkey). Other keys (press hotkeys, Esc) are left to the caller.
    pub fn on_key(&mut self, bindings: &InputBindings, key: KeyCode, pressed: bool) -> bool {
        let hold = bindings.key_hotkey(key).is_some_and(RunnerHotkey::is_hold);
        if !hold && !bindings.is_game_key(key) {
            return false;
        }
        if !pressed {
            self.held.retain(|k| *k != key);
        } else if !self.held.contains(&key) {
            self.held.push(key);
        }
        true
    }

    /// Forgets every held key (focus lost or menu opened: no key-up will come).
    pub fn release_all(&mut self) {
        self.held.clear();
    }

    /// Port-1 SNES buttons from the held keys.
    pub fn mask(&self, bindings: &InputBindings) -> u16 {
        self.held
            .iter()
            .fold(0, |mask, key| mask | bindings.key_mask(*key))
    }

    /// Port-1 stick directions from the held keys.
    pub fn sticks(&self, bindings: &InputBindings) -> u8 {
        self.held
            .iter()
            .fold(0, |mask, key| mask | bindings.key_sticks(*key))
    }

    /// Whether a key bound to `hotkey` is held (hold hotkeys).
    pub fn hotkey_held(&self, bindings: &InputBindings, hotkey: RunnerHotkey) -> bool {
        self.held
            .iter()
            .any(|key| bindings.key_hotkey(*key) == Some(hotkey))
    }
}

#[cfg(test)]
mod tests {
    use config_layers::InputConfig;

    use super::*;
    use crate::joypad_state_snapshot::JoypadButton;

    #[test]
    fn two_keys_on_one_button_release_independently() {
        let mut config = InputConfig::default();
        config
            .keyboard
            .insert("b".into(), vec!["KeyZ".into(), "KeyO".into()]);
        let (bindings, _) = InputBindings::from_config(&config, None);
        let mut keys = KeyboardState::default();
        assert!(keys.on_key(&bindings, KeyCode::KeyZ, true));
        assert!(keys.on_key(&bindings, KeyCode::KeyO, true));
        assert!(keys.on_key(&bindings, KeyCode::KeyZ, false));
        assert_eq!(
            keys.mask(&bindings),
            JoypadButton::B.mask(),
            "O still holds B"
        );
        keys.on_key(&bindings, KeyCode::KeyO, false);
        assert_eq!(keys.mask(&bindings), 0);
    }

    #[test]
    fn stick_action_keys_are_game_keys_that_hold_directions() {
        use crate::analog_stick_input::StickDirection;
        let (bindings, _) = InputBindings::from_config(&InputConfig::default(), None);
        let mut keys = KeyboardState::default();
        assert!(keys.on_key(&bindings, KeyCode::KeyL, true), "lstick_right");
        assert!(keys.on_key(&bindings, KeyCode::KeyT, true), "rstick_up");
        let held = StickDirection::LeftRight.mask() | StickDirection::RightUp.mask();
        assert_eq!(keys.sticks(&bindings), held);
        assert_eq!(keys.mask(&bindings), 0, "no RetroPad button");
        keys.on_key(&bindings, KeyCode::KeyL, false);
        assert_eq!(keys.sticks(&bindings), StickDirection::RightUp.mask());
    }

    #[test]
    fn hold_keys_come_from_config_and_press_hotkeys_pass_through() {
        let mut config = InputConfig::default();
        config
            .hotkeys_keyboard
            .insert("fast_forward_hold".into(), vec!["Tab".into()]);
        let (bindings, _) = InputBindings::from_config(&config, None);
        let mut keys = KeyboardState::default();
        let ff = RunnerHotkey::FastForwardHold;
        assert!(
            !keys.on_key(&bindings, KeyCode::Backquote, true),
            "old FF key"
        );
        assert!(keys.on_key(&bindings, KeyCode::Tab, true));
        assert!(keys.hotkey_held(&bindings, ff));
        assert_eq!(keys.mask(&bindings), 0, "hold keys never reach the core");
        assert!(
            !keys.on_key(&bindings, KeyCode::F1, true),
            "save_state is a press hotkey"
        );
        assert!(!keys.on_key(&bindings, KeyCode::Escape, true));
        keys.release_all();
        assert!(!keys.hotkey_held(&bindings, ff));
    }
}
