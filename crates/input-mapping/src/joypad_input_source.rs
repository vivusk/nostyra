//! `InputSource` handed to the core: merges winit keyboard state (port 0) and
//! SDL gamepads (buttons, analog sticks) into one snapshot, taken when the core
//! polls (inside `retro_run`, right before it reads input → ≤ 1 frame latency). Bindings come
//! from the config through [`crate::InputBindings`], owned by the pad manager.

use libretro_host::{InputSource, RETRO_DEVICE_ANALOG, RETRO_DEVICE_JOYPAD, RETRO_DEVICE_POINTER};
use winit::keyboard::KeyCode;

use crate::input_bindings::InputBindings;
use crate::joypad_state_snapshot::JoypadStateSnapshot;
use crate::keyboard_state::KeyboardState;
use crate::pointer_state::PointerState;
use crate::runner_hotkeys::RunnerHotkey;
use crate::sdl_gamepad_manager::SdlGamepadManager;
use crate::turbo_button::TurboButton;

pub struct JoypadInputSource {
    keyboard: KeyboardState,
    gamepads: SdlGamepadManager,
    snapshot: JoypadStateSnapshot,
    turbo_a: TurboButton,
    /// Mouse pointer; `Some` only for systems with a touch screen.
    pointer: Option<PointerState>,
}

impl JoypadInputSource {
    pub fn new(gamepads: SdlGamepadManager) -> Self {
        Self {
            keyboard: KeyboardState::default(),
            gamepads,
            snapshot: JoypadStateSnapshot::default(),
            turbo_a: TurboButton::default(),
            pointer: None,
        }
    }

    /// Hotplug handling lives in the manager; the runner forwards SDL events here.
    pub fn gamepads_mut(&mut self) -> &mut SdlGamepadManager {
        &mut self.gamepads
    }

    /// The bindings in use (pad hotkey lookups by the runner).
    pub fn bindings(&self) -> &InputBindings {
        self.gamepads.bindings()
    }

    /// Feeds a winit key press/release; returns true if it was consumed (a game
    /// binding or a held fast-forward / rewind key).
    pub fn on_key(&mut self, key: KeyCode, pressed: bool) -> bool {
        self.keyboard.on_key(self.gamepads.bindings(), key, pressed)
    }

    /// Press hotkey of a key not consumed by [`Self::on_key`] (Esc → pause menu always).
    pub fn key_hotkey(&self, key: KeyCode) -> Option<RunnerHotkey> {
        self.bindings().key_hotkey(key)
    }

    /// Releases all keyboard buttons (window lost focus → no stuck keys).
    pub fn release_keyboard(&mut self) {
        self.keyboard.release_all();
    }

    /// Fast-forward is held: a bound key or pad input (SDL state is as of the last
    /// event pump).
    pub fn fast_forward_held(&self) -> bool {
        self.hotkey_held(RunnerHotkey::FastForwardHold)
    }

    /// Rewind is held: a bound key or pad input (as of the last pump).
    pub fn rewind_held(&self) -> bool {
        self.hotkey_held(RunnerHotkey::RewindHold)
    }

    /// Player 1 A auto-fire (`turbo_a_toggle` hotkey).
    pub fn turbo_a_mut(&mut self) -> &mut TurboButton {
        &mut self.turbo_a
    }

    /// Sets the pointer the core reads (the runner, for touch-screen systems only;
    /// until then pointer queries answer 0).
    pub fn set_pointer(&mut self, pointer: PointerState) {
        self.pointer = Some(pointer);
    }

    fn hotkey_held(&self, hotkey: RunnerHotkey) -> bool {
        self.keyboard.hotkey_held(self.bindings(), hotkey) || self.gamepads.hotkey_held(hotkey)
    }
}

impl InputSource for JoypadInputSource {
    fn poll(&mut self) {
        self.snapshot = JoypadStateSnapshot::default();
        let keyboard_mask = self.keyboard.mask(self.gamepads.bindings());
        self.snapshot.press_mask(0, keyboard_mask);
        let keyboard_sticks = self.keyboard.sticks(self.gamepads.bindings());
        self.snapshot.hold_sticks(0, keyboard_sticks);
        self.gamepads.read_into(&mut self.snapshot);
        self.turbo_a.press_into(&mut self.snapshot);
    }

    /// Joypad buttons; analog sticks (systems without them, and stick actions
    /// unbound, leave the snapshot centred → 0); the port-0 pointer of touch-screen
    /// systems; other devices → 0.
    fn state(&mut self, port: u32, device: u32, index: u32, id: u32) -> i16 {
        reply(
            &self.snapshot,
            self.pointer.as_ref(),
            port,
            device,
            index,
            id,
        )
    }
}

fn reply(
    snapshot: &JoypadStateSnapshot,
    pointer: Option<&PointerState>,
    port: u32,
    device: u32,
    index: u32,
    id: u32,
) -> i16 {
    match (device, pointer) {
        (RETRO_DEVICE_JOYPAD, _) => snapshot.state(port as usize, id),
        (RETRO_DEVICE_ANALOG, _) => snapshot.analog_state(port as usize, index, id),
        (RETRO_DEVICE_POINTER, Some(pointer)) if port == 0 => pointer.reply(index, id),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analog_stick_input::{StickDirection, ANALOG_MAX};
    use crate::joypad_state_snapshot::JoypadButton;

    #[test]
    fn analog_device_reads_sticks_and_joypad_reads_buttons() {
        let mut snap = JoypadStateSnapshot::default();
        snap.press(0, JoypadButton::B);
        snap.set_axes(0, [9_000, 0, 0, 0]);
        snap.hold_sticks(0, StickDirection::RightDown.mask());
        assert_eq!(reply(&snap, None, 0, RETRO_DEVICE_JOYPAD, 0, 0), 1);
        assert_eq!(reply(&snap, None, 0, RETRO_DEVICE_ANALOG, 0, 0), 9_000);
        assert_eq!(reply(&snap, None, 0, RETRO_DEVICE_ANALOG, 1, 1), ANALOG_MAX);
        assert_eq!(reply(&snap, None, 0, 3, 0, 0), 0, "mouse");
        assert_eq!(
            reply(&snap, None, 0, RETRO_DEVICE_POINTER, 0, 15),
            0,
            "no touch screen"
        );
    }

    #[test]
    fn pointer_answers_port_zero_of_touch_screen_systems() {
        let snap = JoypadStateSnapshot::default();
        let pointer = PointerState {
            x: 1_000,
            y: -2_000,
            pressed: true,
            offscreen: false,
        };
        let touch = Some(&pointer);
        assert_eq!(reply(&snap, touch, 0, RETRO_DEVICE_POINTER, 0, 0), 1_000);
        assert_eq!(reply(&snap, touch, 0, RETRO_DEVICE_POINTER, 0, 1), -2_000);
        assert_eq!(reply(&snap, touch, 0, RETRO_DEVICE_POINTER, 0, 2), 1);
        assert_eq!(
            reply(&snap, touch, 1, RETRO_DEVICE_POINTER, 0, 2),
            0,
            "port 1"
        );
        assert_eq!(
            reply(&snap, touch, 0, RETRO_DEVICE_JOYPAD, 0, 0),
            0,
            "pad unaffected"
        );
    }

    #[test]
    fn systems_without_analog_sticks_reply_centred() {
        // The pad manager never writes axes for them and their configs drop the
        // stick actions, so the snapshot stays centred.
        let mut snap = JoypadStateSnapshot::default();
        snap.press_mask(0, JoypadButton::Right.mask());
        for (index, id) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            assert_eq!(reply(&snap, None, 0, RETRO_DEVICE_ANALOG, index, id), 0);
        }
    }
}
