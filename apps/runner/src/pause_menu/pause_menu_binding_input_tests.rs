//! Pause-menu input with config bindings: a pad or keyboard `pause_menu` binding
//! acts as Back in the open menu (the pad binding also opens it) without
//! stealing fixed navigation, other pad inputs pass to pad hotkeys only while
//! closed, and a remapped Start held at resume is swallowed like Enter.

use std::time::Instant;

use input_mapping::{GamepadInput, InputBindings, JoypadButton};
use sdl3::gamepad::Button;
use winit::keyboard::KeyCode;

use super::menu_types::{KeyRoute, NavEvent, PadRoute};
use super::pause_menu_input::PauseMenuInput;

fn pad(button: Button) -> GamepadInput {
    GamepadInput::Button(button)
}

#[test]
fn pad_menu_binding_opens_and_acts_as_back_but_never_steals_navigation() {
    let mut input = PauseMenuInput::default();
    let t0 = Instant::now();
    let stick = pad(Button::RightStick);
    assert_eq!(
        input.on_pad_input(stick, true, true, false, t0),
        PadRoute::Open
    );
    assert_eq!(
        input.on_pad_input(stick, false, true, true, t0),
        PadRoute::Ignore
    );
    assert_eq!(
        input.on_pad_input(stick, true, true, true, t0),
        PadRoute::Nav(NavEvent::Back)
    );
    assert_eq!(
        input.on_pad_input(pad(Button::South), true, true, true, t0),
        PadRoute::Nav(NavEvent::Confirm),
        "fixed navigation wins over a menu binding"
    );
    // Other inputs (e.g. a pad hotkey) are ignored while open, passed while closed.
    assert_eq!(
        input.on_pad_input(GamepadInput::LeftTrigger, true, false, true, t0),
        PadRoute::Ignore
    );
    assert_eq!(
        input.on_pad_input(GamepadInput::LeftTrigger, true, false, false, t0),
        PadRoute::Pass
    );
    assert_eq!(
        input.on_pad_input(GamepadInput::LeftTrigger, false, false, false, t0),
        PadRoute::Ignore,
        "releases never fire hotkeys"
    );
}

#[test]
fn remapped_start_held_at_resume_is_swallowed_until_released() {
    let mut config = config_layers::InputConfig::default();
    config.keyboard.insert("start".into(), vec!["KeyO".into()]);
    let (bindings, _) = InputBindings::from_config(&config, None);
    assert_eq!(bindings.key_mask(KeyCode::KeyO), JoypadButton::Start.mask());
    let mut input = PauseMenuInput::default();
    let t0 = Instant::now();
    input.on_open();
    assert_eq!(
        input.on_key(KeyCode::KeyO, true, false, false, true, t0),
        KeyRoute::Consumed
    );
    input.on_close();
    assert_eq!(
        input.on_key(KeyCode::KeyO, true, true, false, false, t0),
        KeyRoute::Consumed,
        "held remapped Start never reaches the game"
    );
    assert_eq!(
        input.on_key(KeyCode::KeyO, false, false, false, false, t0),
        KeyRoute::Consumed
    );
    assert_eq!(
        input.on_key(KeyCode::KeyO, true, false, false, false, t0),
        KeyRoute::Pass
    );
}

#[test]
fn keyboard_menu_binding_closes_the_menu_and_f10_is_not_hard_coded() {
    let mut input = PauseMenuInput::default();
    let t0 = Instant::now();
    input.on_open();
    assert_eq!(
        input.on_key(KeyCode::F9, true, false, true, true, t0),
        KeyRoute::Nav(NavEvent::Back),
        "a key bound to pause_menu acts as Back"
    );
    assert_eq!(
        input.on_key(KeyCode::F10, true, false, false, true, t0),
        KeyRoute::Consumed,
        "F10 closes only while bound"
    );
    assert_eq!(
        input.on_key(KeyCode::Enter, true, false, true, true, t0),
        KeyRoute::Nav(NavEvent::Confirm),
        "fixed navigation wins over a menu binding"
    );
    assert_eq!(
        input.on_key(KeyCode::Escape, true, false, false, true, t0),
        KeyRoute::Nav(NavEvent::Back),
        "Esc stays fixed"
    );
}
