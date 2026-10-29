//! Keyboard and gamepad → menu navigation, with our own hold-to-repeat (400 ms delay,
//! then every 80 ms; OS key repeats are ignored so keyboard and pad feel the same),
//! the Select+Start hold chord (0.5 s) that opens the menu, and resume hygiene:
//! keys still held when the menu closes are swallowed until released, so Confirm
//! (Enter = SNES Start) never leaks into the game. Menu navigation keys / pad
//! buttons and Guide are fixed (never from bindings), so the menu stays usable
//! whatever the user binds.

use std::time::{Duration, Instant};

use input_mapping::GamepadInput;
use sdl3::gamepad::Button;
use winit::keyboard::KeyCode;

use super::menu_types::{key_nav, pad_nav, KeyRoute, NavEvent, PadRoute};

pub const REPEAT_DELAY: Duration = Duration::from_millis(400);
pub const REPEAT_RATE: Duration = Duration::from_millis(80);
/// Select+Start must be held this long to open the menu (both reach the core meanwhile).
pub const CHORD_HOLD: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeldInput {
    Key(KeyCode),
    Pad(GamepadInput),
}

struct Repeat {
    input: HeldInput,
    nav: NavEvent,
    next: Instant,
}

pub struct PauseMenuInput {
    /// Keys pressed while the menu is open and not yet released.
    held_keys: Vec<KeyCode>,
    /// Keys held when the menu closed: eaten until released.
    swallowed: Vec<KeyCode>,
    repeat: Option<Repeat>,
    chord_since: Option<Instant>,
    /// The chord must be released before it can open the menu again.
    chord_armed: bool,
}

impl Default for PauseMenuInput {
    fn default() -> Self {
        Self {
            held_keys: Vec::new(),
            swallowed: Vec::new(),
            repeat: None,
            chord_since: None,
            chord_armed: true,
        }
    }
}

impl PauseMenuInput {
    /// Menu opened: keys swallowed from a previous close are still physically held,
    /// so they are tracked as held from now on.
    pub fn on_open(&mut self) {
        self.held_keys.append(&mut self.swallowed);
        (self.repeat, self.chord_since) = (None, None);
    }

    /// Menu closed: everything still held is swallowed until released.
    pub fn on_close(&mut self) {
        self.swallowed.append(&mut self.held_keys);
        (self.repeat, self.chord_armed) = (None, false);
    }

    /// Window lost focus: no key-up events will come for held keys.
    pub fn release_keys(&mut self) {
        self.held_keys.clear();
        self.swallowed.clear();
        self.repeat = None;
    }

    /// Routes a winit key event (`open` = menu shown; `menu_binding` = the key is
    /// bound to the `pause_menu` hotkey, so it closes the menu like the pad
    /// binding does: fixed navigation first, then Back).
    pub fn on_key(
        &mut self,
        key: KeyCode,
        pressed: bool,
        repeat: bool,
        menu_binding: bool,
        open: bool,
        now: Instant,
    ) -> KeyRoute {
        if !open {
            let Some(index) = self.swallowed.iter().position(|k| *k == key) else {
                return KeyRoute::Pass;
            };
            if !pressed {
                self.swallowed.swap_remove(index);
            }
            return KeyRoute::Consumed;
        }
        if !pressed {
            self.held_keys.retain(|k| *k != key);
            self.stop_repeat(HeldInput::Key(key));
            return KeyRoute::Consumed;
        }
        if !self.held_keys.contains(&key) {
            self.held_keys.push(key);
        }
        let nav = key_nav(key).or(menu_binding.then_some(NavEvent::Back));
        match nav.filter(|_| !repeat) {
            Some(nav) => {
                self.start_repeat(HeldInput::Key(key), nav, now);
                KeyRoute::Nav(nav)
            }
            None => KeyRoute::Consumed,
        }
    }

    /// Routes a pad input event (`open` = menu shown; `menu_binding` = the input
    /// is bound to the pad `pause_menu` hotkey). Guide or the menu binding opens
    /// the menu; while open, fixed navigation first, then the menu binding acts
    /// as Back (like Guide), so a binding can never take Confirm away.
    pub fn on_pad_input(
        &mut self,
        input: GamepadInput,
        pressed: bool,
        menu_binding: bool,
        open: bool,
        now: Instant,
    ) -> PadRoute {
        if !pressed {
            self.stop_repeat(HeldInput::Pad(input));
            return PadRoute::Ignore;
        }
        if !open {
            let guide = input == GamepadInput::Button(Button::Guide);
            return if guide || menu_binding {
                PadRoute::Open
            } else {
                PadRoute::Pass
            };
        }
        let nav = match input {
            GamepadInput::Button(button) => pad_nav(button),
            _ => None,
        };
        match nav.or(menu_binding.then_some(NavEvent::Back)) {
            Some(nav) => {
                self.start_repeat(HeldInput::Pad(input), nav, now);
                PadRoute::Nav(nav)
            }
            None => PadRoute::Ignore,
        }
    }

    /// Once per redraw: whether the Select+Start chord has been held long enough.
    pub fn poll_chord(&mut self, held: bool, open: bool, now: Instant) -> bool {
        if !held {
            (self.chord_since, self.chord_armed) = (None, true);
            return false;
        }
        if open || !self.chord_armed {
            return false;
        }
        let since = *self.chord_since.get_or_insert(now);
        if now.duration_since(since) < CHORD_HOLD {
            return false;
        }
        (self.chord_since, self.chord_armed) = (None, false);
        true
    }

    /// Once per redraw while open: a repeated direction when one is held long enough.
    pub fn poll_repeat(&mut self, now: Instant) -> Option<NavEvent> {
        let repeat = self.repeat.as_mut().filter(|r| now >= r.next)?;
        repeat.next = (repeat.next + REPEAT_RATE).max(now);
        Some(repeat.nav)
    }

    fn start_repeat(&mut self, input: HeldInput, nav: NavEvent, now: Instant) {
        let directional = !matches!(nav, NavEvent::Confirm | NavEvent::Back);
        self.repeat = directional.then_some(Repeat {
            input,
            nav,
            next: now + REPEAT_DELAY,
        });
    }

    fn stop_repeat(&mut self, input: HeldInput) {
        if self.repeat.as_ref().is_some_and(|r| r.input == input) {
            self.repeat = None;
        }
    }
}
