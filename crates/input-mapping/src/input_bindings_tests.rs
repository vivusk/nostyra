//! `InputBindings` tests: the default config reproduces the M1–M3 tables, and
//! remaps, multi-button keys, unbinds, pad hotkeys and fixed Esc behave.

use config_layers::InputConfig;
use sdl3::gamepad::Button;
use winit::keyboard::KeyCode;

use super::*;
use crate::gamepad_input_names::GamepadInput as Pad;
use crate::joypad_state_snapshot::JoypadButton as J;

fn defaults() -> InputBindings {
    let (bindings, warnings) = InputBindings::from_config(&InputConfig::default(), None);
    assert!(warnings.is_empty(), "{warnings:?}");
    bindings
}

fn binds(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| (*s).to_string()).collect()
}

/// Mask of one pad with exactly `inputs` held.
fn pad_mask_with(bindings: &InputBindings, inputs: &[Pad]) -> u16 {
    bindings.pad_mask(|input| inputs.contains(&input))
}

#[test]
fn default_config_equals_the_old_hard_coded_tables() {
    let b = defaults();
    let old_keyboard = [
        (KeyCode::ArrowUp, J::Up),
        (KeyCode::ArrowDown, J::Down),
        (KeyCode::ArrowLeft, J::Left),
        (KeyCode::ArrowRight, J::Right),
        (KeyCode::KeyZ, J::B),
        (KeyCode::KeyX, J::A),
        (KeyCode::KeyA, J::Y),
        (KeyCode::KeyS, J::X),
        (KeyCode::KeyQ, J::L),
        (KeyCode::KeyW, J::R),
        (KeyCode::Enter, J::Start),
        (KeyCode::ShiftRight, J::Select),
        (KeyCode::KeyE, J::L2), // second shoulder pair (PlayStation), added in M8
        (KeyCode::KeyR, J::R2),
    ];
    for (key, button) in old_keyboard {
        assert_eq!(b.key_mask(key), button.mask(), "{key:?}");
    }
    assert_eq!(
        b.keyboard.len(),
        old_keyboard.len() + 8,
        "+ I/K/J/L, T/G/F/H sticks"
    );
    let old_gamepad = [
        (Button::South, J::B),
        (Button::East, J::A),
        (Button::West, J::Y),
        (Button::North, J::X),
        (Button::LeftShoulder, J::L),
        (Button::RightShoulder, J::R),
        (Button::Start, J::Start),
        (Button::Back, J::Select),
        (Button::DPadUp, J::Up),
        (Button::DPadDown, J::Down),
        (Button::DPadLeft, J::Left),
        (Button::DPadRight, J::Right),
    ];
    for (button, joypad) in old_gamepad {
        assert_eq!(pad_mask_with(&b, &[Pad::Button(button)]), joypad.mask());
    }
    let clicks = [(Button::LeftStick, J::L3), (Button::RightStick, J::R3)]; // M13
    for (button, joypad) in clicks {
        assert_eq!(pad_mask_with(&b, &[Pad::Button(button)]), joypad.mask());
    }
    assert_eq!(b.gamepad.len(), old_gamepad.len() + clicks.len());
    let old_hotkeys = [
        (KeyCode::F1, RunnerHotkey::SaveState),
        (KeyCode::F2, RunnerHotkey::PrevSlot),
        (KeyCode::F3, RunnerHotkey::NextSlot),
        (KeyCode::F4, RunnerHotkey::LoadState),
        (KeyCode::F11, RunnerHotkey::ToggleFullscreen),
        (KeyCode::Escape, RunnerHotkey::PauseMenu),
        (KeyCode::F10, RunnerHotkey::PauseMenu),
        (KeyCode::Backquote, RunnerHotkey::FastForwardHold),
        (KeyCode::Backspace, RunnerHotkey::RewindHold),
        (KeyCode::KeyP, RunnerHotkey::TurboAToggle),
    ];
    for (key, hotkey) in old_hotkeys {
        assert_eq!(b.key_hotkey(key), Some(hotkey), "{key:?}");
    }
    assert_eq!(b.keyboard_hotkeys.len(), old_hotkeys.len());
    assert_eq!(
        b.pad_hotkey(Pad::RightTrigger),
        Some(RunnerHotkey::FastForwardHold)
    );
    assert_eq!(
        b.pad_hotkey(Pad::LeftTrigger),
        Some(RunnerHotkey::RewindHold)
    );
    assert_eq!(
        b.gamepad_hotkeys.len(),
        2,
        "other pad hotkeys start unbound"
    );
    assert_eq!(
        b.pad_hotkey(Pad::Button(Button::Guide)),
        None,
        "Guide is fixed"
    );
}

#[test]
fn default_hotkeys_do_not_overlap_game_bindings() {
    let b = defaults();
    for key in b.keyboard_hotkeys.keys() {
        assert_eq!(b.key_mask(*key), 0, "{key:?}");
    }
    for (input, _) in &b.gamepad_hotkeys {
        assert_eq!(pad_mask_with(&b, &[*input]), 0, "{input:?}");
    }
}

#[test]
fn one_key_on_two_buttons_sets_both_and_unbound_action_never_fires() {
    let mut config = InputConfig::default();
    config.keyboard.insert("a".into(), binds(&["KeyZ"])); // KeyZ = B and A
    config.keyboard.insert("start".into(), binds(&[]));
    let (b, warnings) = InputBindings::from_config(&config, None);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(b.key_mask(KeyCode::KeyZ), J::B.mask() | J::A.mask());
    assert_eq!(b.key_mask(KeyCode::KeyX), 0, "old A key unbound");
    assert_eq!(b.key_mask(KeyCode::Enter), 0, "start = [] is unbound");
    let all_keys = b.keyboard.values().fold(0, |m, bits| m | bits);
    assert_eq!(all_keys & u32::from(J::Start.mask()), 0);
}

#[test]
fn pad_hotkey_on_right_stick_is_a_hotkey_not_a_core_button() {
    let mut config = InputConfig::default();
    config
        .hotkeys_gamepad
        .insert("save_state".into(), binds(&["right_stick"]));
    config
        .gamepad
        .insert("r".into(), binds(&["right_stick", "right_shoulder"]));
    let (b, warnings) = InputBindings::from_config(&config, None);
    assert_eq!(warnings.len(), 2, "hotkey wins over r and r3: {warnings:?}");
    let stick = Pad::Button(Button::RightStick);
    assert_eq!(b.pad_hotkey(stick), Some(RunnerHotkey::SaveState));
    assert_eq!(pad_mask_with(&b, &[stick]), 0, "never reaches the core");
    assert_eq!(
        pad_mask_with(&b, &[Pad::Button(Button::RightShoulder)]),
        J::R.mask()
    );
}

#[test]
fn trigger_bound_as_game_button_presses_it_and_holds_follow_config() {
    let mut config = InputConfig::default();
    config.gamepad.insert("r".into(), binds(&["right_trigger"]));
    config
        .hotkeys_gamepad
        .insert("fast_forward_hold".into(), binds(&["left_stick"]));
    // An SNES session: the free right trigger is not taken by the unused `r2`.
    let snes_actions = &config_layers::PAD_ACTIONS[..12];
    let (b, warnings) = InputBindings::from_config(&config, Some(snes_actions));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(pad_mask_with(&b, &[Pad::RightTrigger]), J::R.mask());
    let left_stick = Pad::Button(Button::LeftStick);
    let ff = RunnerHotkey::FastForwardHold;
    assert!(b.pad_hotkey_held(ff, |i| i == left_stick));
    assert!(!b.pad_hotkey_held(ff, |i| i == Pad::RightTrigger));
    assert!(b.pad_hotkey_held(RunnerHotkey::RewindHold, |i| i == Pad::LeftTrigger));
}

#[test]
fn escape_opens_the_menu_even_when_pause_menu_is_unbound() {
    let mut config = InputConfig::default();
    config
        .hotkeys_keyboard
        .insert("pause_menu".into(), binds(&[]));
    let (b, _) = InputBindings::from_config(&config, None);
    assert_eq!(b.key_hotkey(KeyCode::Escape), Some(RunnerHotkey::PauseMenu));
    assert_eq!(b.key_hotkey(KeyCode::F10), None);
}

#[test]
fn unconvertible_names_warn_instead_of_panicking() {
    let mut config = InputConfig::default();
    config
        .keyboard
        .insert("b".into(), binds(&["NoSuchKey", "KeyO"]));
    config.gamepad.insert("turbo".into(), binds(&["south"]));
    let (b, warnings) = InputBindings::from_config(&config, None);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("input_keyboard.b"), "{warnings:?}");
    assert_eq!(b.key_mask(KeyCode::KeyO), J::B.mask());
}
