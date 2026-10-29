//! Keyboard input names ↔ winit `KeyCode`. Names are the `KeyCode` variant
//! names (physical scancodes) listed in `config_layers::KEYBOARD_KEY_NAMES`;
//! the table is generated from the variant identifiers, so a name can never
//! drift from the key it stands for. No winit `serde` feature needed.

use winit::keyboard::KeyCode;

/// Builds `KEY_TABLE: [(name, KeyCode)]` from bare `KeyCode` variant names.
macro_rules! key_table {
    ($($key:ident),* $(,)?) => {
        const KEY_TABLE: &[(&str, KeyCode)] = &[$((stringify!($key), KeyCode::$key)),*];
    };
}

#[rustfmt::skip]
key_table!(
    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM,
    KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Insert, Delete, Home, End, PageUp, PageDown,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8,
    Numpad9, NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadDecimal,
    NumpadEnter, NumLock,
    ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight, SuperLeft, SuperRight,
    Backquote, Minus, Equal, BracketLeft, BracketRight, Backslash, IntlBackslash,
    Semicolon, Quote, Comma, Period, Slash,
    Space, Enter, Tab, Backspace, Escape, CapsLock, ContextMenu, PrintScreen,
    ScrollLock, Pause,
);

/// The key a config name stands for (`None` = not a supported key name).
pub fn key_code_from_name(name: &str) -> Option<KeyCode> {
    KEY_TABLE
        .iter()
        .find(|(key_name, _)| *key_name == name)
        .map(|(_, key)| *key)
}

/// Config name of a key (`None` = key outside the supported set).
pub fn key_code_name(key: KeyCode) -> Option<&'static str> {
    KEY_TABLE
        .iter()
        .find(|(_, code)| *code == key)
        .map(|(name, _)| *name)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use config_layers::KEYBOARD_KEY_NAMES;

    use super::*;

    #[test]
    fn every_config_key_name_converts_and_round_trips() {
        for name in KEYBOARD_KEY_NAMES {
            let key = key_code_from_name(name);
            assert!(key.is_some(), "{name} has no KeyCode");
            assert_eq!(key.and_then(key_code_name), Some(*name));
        }
    }

    #[test]
    fn table_matches_config_names_exactly_without_duplicates() {
        assert_eq!(KEY_TABLE.len(), KEYBOARD_KEY_NAMES.len());
        let names: HashSet<_> = KEY_TABLE.iter().map(|(name, _)| *name).collect();
        let keys: HashSet<_> = KEY_TABLE.iter().map(|(_, key)| *key).collect();
        assert_eq!(names.len(), KEY_TABLE.len());
        assert_eq!(keys.len(), KEY_TABLE.len());
        assert!(names.iter().all(|name| KEYBOARD_KEY_NAMES.contains(name)));
    }

    #[test]
    fn unknown_names_and_keys_are_none() {
        assert_eq!(key_code_from_name("Meta"), None);
        assert_eq!(key_code_from_name("keyz"), None);
        assert_eq!(key_code_name(KeyCode::F24), None);
    }
}
