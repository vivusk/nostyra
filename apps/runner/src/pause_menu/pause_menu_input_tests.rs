//! Input adapter tests with a fake clock: repeat timing, the Select+Start chord,
//! and keys held at resume never reaching the game.

use std::time::{Duration, Instant};

use input_mapping::GamepadInput;
use sdl3::gamepad::Button;
use winit::keyboard::KeyCode;

use super::menu_types::{KeyRoute, NavEvent, PadRoute};
use super::pause_menu_input::{PauseMenuInput, CHORD_HOLD, REPEAT_DELAY, REPEAT_RATE};

const MS: Duration = Duration::from_millis(1);

fn pad(button: Button) -> GamepadInput {
    GamepadInput::Button(button)
}

#[test]
fn held_direction_repeats_after_delay_at_rate() {
    let mut input = PauseMenuInput::default();
    let t0 = Instant::now();
    input.on_open();
    assert_eq!(
        input.on_key(KeyCode::ArrowDown, true, false, false, true, t0),
        KeyRoute::Nav(NavEvent::Down)
    );
    assert_eq!(input.poll_repeat(t0 + REPEAT_DELAY - MS), None);
    assert_eq!(input.poll_repeat(t0 + REPEAT_DELAY), Some(NavEvent::Down));
    assert_eq!(input.poll_repeat(t0 + REPEAT_DELAY + MS), None);
    assert_eq!(
        input.poll_repeat(t0 + REPEAT_DELAY + REPEAT_RATE),
        Some(NavEvent::Down)
    );
    // OS key repeats are ignored (our own timer drives repeats).
    assert_eq!(
        input.on_key(KeyCode::ArrowDown, true, true, false, true, t0),
        KeyRoute::Consumed
    );
    assert_eq!(
        input.on_key(KeyCode::ArrowDown, false, false, false, true, t0),
        KeyRoute::Consumed
    );
    assert_eq!(
        input.poll_repeat(t0 + Duration::from_secs(5)),
        None,
        "released: no repeat"
    );
}

#[test]
fn confirm_does_not_repeat_and_pad_repeats_like_keys() {
    let mut input = PauseMenuInput::default();
    let t0 = Instant::now();
    input.on_open();
    assert_eq!(
        input.on_pad_input(pad(Button::South), true, false, true, t0),
        PadRoute::Nav(NavEvent::Confirm)
    );
    assert_eq!(input.poll_repeat(t0 + Duration::from_secs(1)), None);
    assert_eq!(
        input.on_pad_input(pad(Button::DPadLeft), true, false, true, t0),
        PadRoute::Nav(NavEvent::Left)
    );
    assert_eq!(input.poll_repeat(t0 + REPEAT_DELAY), Some(NavEvent::Left));
    input.on_pad_input(pad(Button::DPadLeft), false, false, true, t0);
    assert_eq!(input.poll_repeat(t0 + REPEAT_DELAY + REPEAT_RATE), None);
    assert_eq!(
        input.on_pad_input(pad(Button::East), true, false, true, t0),
        PadRoute::Nav(NavEvent::Back)
    );
}

#[test]
fn guide_opens_only_while_closed() {
    let mut input = PauseMenuInput::default();
    let t0 = Instant::now();
    assert_eq!(
        input.on_pad_input(pad(Button::Guide), true, false, false, t0),
        PadRoute::Open
    );
    assert_eq!(
        input.on_pad_input(pad(Button::South), true, false, false, t0),
        PadRoute::Pass,
        "closed: free for pad hotkeys"
    );
    assert_eq!(
        input.on_pad_input(pad(Button::Guide), true, false, true, t0),
        PadRoute::Nav(NavEvent::Back)
    );
}

#[test]
fn chord_opens_after_half_a_second_and_needs_a_release() {
    let mut input = PauseMenuInput::default();
    let t0 = Instant::now();
    assert!(!input.poll_chord(true, false, t0));
    assert!(!input.poll_chord(true, false, t0 + CHORD_HOLD - MS));
    assert!(input.poll_chord(true, false, t0 + CHORD_HOLD));
    input.on_open();
    input.on_close();
    // Still held after the menu closed: must not reopen.
    assert!(!input.poll_chord(true, false, t0 + 4 * CHORD_HOLD));
    assert!(!input.poll_chord(false, false, t0 + 4 * CHORD_HOLD));
    let t1 = t0 + 5 * CHORD_HOLD;
    assert!(!input.poll_chord(true, false, t1));
    assert!(input.poll_chord(true, false, t1 + CHORD_HOLD));
}

#[test]
fn enter_held_at_resume_is_swallowed_until_released() {
    let mut input = PauseMenuInput::default();
    let t0 = Instant::now();
    input.on_open();
    assert_eq!(
        input.on_key(KeyCode::Enter, true, false, false, true, t0),
        KeyRoute::Nav(NavEvent::Confirm)
    );
    input.on_close(); // "Resume" confirmed with Enter still down
                      // OS repeat of the held Enter (= SNES Start) must not reach the game.
    assert_eq!(
        input.on_key(KeyCode::Enter, true, true, false, false, t0),
        KeyRoute::Consumed
    );
    assert_eq!(
        input.on_key(KeyCode::Enter, false, false, false, false, t0),
        KeyRoute::Consumed
    );
    assert_eq!(
        input.on_key(KeyCode::Enter, true, false, false, false, t0),
        KeyRoute::Pass,
        "fresh press"
    );
    assert_eq!(
        input.on_key(KeyCode::KeyZ, true, false, false, false, t0),
        KeyRoute::Pass
    );
}

#[test]
fn swallowed_key_released_inside_a_reopened_menu_is_forgotten() {
    let mut input = PauseMenuInput::default();
    let t0 = Instant::now();
    input.on_open();
    input.on_key(KeyCode::Escape, true, false, false, true, t0);
    input.on_close();
    input.on_open(); // reopened (e.g. Guide) before Esc came up
    input.on_key(KeyCode::Escape, false, false, false, true, t0);
    input.on_close();
    assert_eq!(
        input.on_key(KeyCode::Escape, true, false, false, false, t0),
        KeyRoute::Pass
    );
}
