//! Key / pad routing of the pause menu (open, navigation, chord, repeats), split
//! from `pause_menu.rs`.

use std::time::Instant;

use input_mapping::GamepadInput;
use winit::keyboard::KeyCode;

use super::{KeyRoute, NavEvent, PadRoute, PauseMenu};

impl PauseMenu {
    /// Routes a key event; `menu_binding` = bound to the `pause_menu` hotkey.
    pub fn on_key(
        &mut self,
        key: KeyCode,
        pressed: bool,
        repeat: bool,
        menu_binding: bool,
        now: Instant,
    ) -> KeyRoute {
        let open = self.is_open();
        self.input
            .on_key(key, pressed, repeat, menu_binding, open, now)
    }

    /// Routes a pad input event; `menu_binding` = bound to the pad `pause_menu` hotkey.
    pub fn on_pad_input(
        &mut self,
        input: GamepadInput,
        pressed: bool,
        menu_binding: bool,
        now: Instant,
    ) -> PadRoute {
        let open = self.is_open();
        self.input
            .on_pad_input(input, pressed, menu_binding, open, now)
    }

    /// Whether the Select+Start chord (`held` on some pad) should open the menu now.
    pub fn poll_chord(&mut self, held: bool, now: Instant) -> bool {
        let open = self.is_open();
        self.input.poll_chord(held, open, now)
    }

    /// A repeated direction while one is held (menu open only).
    pub fn poll_repeat(&mut self, now: Instant) -> Option<NavEvent> {
        self.is_open()
            .then(|| self.input.poll_repeat(now))
            .flatten()
    }

    /// Window lost focus: forget held keys (no key-up will arrive).
    pub fn release_keys(&mut self) {
        self.input.release_keys();
    }
}
