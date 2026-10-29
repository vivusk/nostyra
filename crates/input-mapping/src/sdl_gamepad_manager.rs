//! Opens/closes SDL gamepads on hotplug and assigns them to player ports
//! (first free slot; port 0 first). Owns the [`InputBindings`]: game buttons and
//! hold hotkeys are read from pad state, and pad input presses / releases
//! (buttons from SDL events, triggers when crossing half travel) are queued for
//! the runner, which routes them to the pause menu (Guide, navigation) or to pad
//! hotkeys. Also reports the fixed Select+Start chord and suppresses buttons still
//! held when the menu closes, so they never reach the core. Raw sticks and the
//! left-stick D-pad follow the system's stick mode (`analog_stick_mode`). Systems
//! with analog triggers also get the trigger travel (`analog_trigger_input`).

use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::joystick::JoystickId;
use sdl3::GamepadSubsystem;

use crate::analog_stick_mode::{left_stick_dpad, raw_stick_axes, AnalogSticks};
use crate::analog_trigger_input::{read_pad_triggers, AnalogInputs};
use crate::gamepad_input_names::{trigger_held, GamepadInput};
use crate::input_bindings::InputBindings;
use crate::joypad_state_snapshot::{JoypadStateSnapshot, PORT_COUNT};
use crate::runner_hotkeys::RunnerHotkey;

pub struct SdlGamepadManager {
    subsystem: GamepadSubsystem,
    bindings: InputBindings,
    ports: [Option<Gamepad>; PORT_COUNT],
    /// Pad input presses / releases since the last `take_pad_events`.
    pad_events: Vec<(GamepadInput, bool)>,
    /// Per port: `[left, right]` trigger past half travel (edge detection).
    triggers: [[bool; 2]; PORT_COUNT],
    /// Per port: SNES buttons held when the pause menu closed, hidden from the core
    /// until each is released.
    suppressed: [u16; PORT_COUNT],
    /// Which sticks go raw to the core (a raw stick is never the D-pad).
    analog_sticks: AnalogSticks,
    /// The system has analog triggers: L2 / R2 pressure to the core.
    analog_triggers: bool,
}

impl SdlGamepadManager {
    /// `analog`: the running system's `SystemSpec.analog_sticks` /
    /// `SystemSpec.analog_triggers`.
    pub fn new(subsystem: GamepadSubsystem, bindings: InputBindings, analog: AnalogInputs) -> Self {
        Self {
            subsystem,
            bindings,
            ports: Default::default(),
            pad_events: Vec::new(),
            triggers: [[false; 2]; PORT_COUNT],
            suppressed: [0; PORT_COUNT],
            analog_sticks: analog.sticks,
            analog_triggers: analog.triggers,
        }
    }

    /// The bindings every pad and the keyboard use.
    pub fn bindings(&self) -> &InputBindings {
        &self.bindings
    }

    /// Handles `GamepadButtonDown` / `GamepadButtonUp` (any pad).
    pub fn on_button(&mut self, button: Button, pressed: bool) {
        self.pad_events
            .push((GamepadInput::Button(button), pressed));
    }

    /// Handles `GamepadAxisMotion`: a trigger crossing half travel on a pad with a
    /// port becomes a press / release event (stick axes are ignored here).
    pub fn on_axis(&mut self, id: JoystickId, axis: Axis, value: i16) {
        let Some(input) = GamepadInput::trigger(axis) else {
            return;
        };
        let Some(port) = self.port_of(id) else {
            return;
        };
        let held = trigger_held(value);
        let state = &mut self.triggers[port][usize::from(input == GamepadInput::RightTrigger)];
        if *state != held {
            *state = held;
            self.pad_events.push((input, held));
        }
    }

    /// Pad input events since the last call, oldest first.
    pub fn take_pad_events(&mut self) -> Vec<(GamepadInput, bool)> {
        std::mem::take(&mut self.pad_events)
    }

    /// Select (Back) and Start held together on one pad: the pause-menu chord
    /// (physical buttons, independent of bindings).
    pub fn menu_chord_held(&self) -> bool {
        self.ports
            .iter()
            .flatten()
            .any(|pad| pad.button(Button::Back) && pad.button(Button::Start))
    }

    /// Hides every SNES button held right now from the core until it is released
    /// (pause menu closed with Confirm still down).
    pub fn suppress_held_buttons(&mut self) {
        for (port, pad) in self.ports.iter().enumerate() {
            self.suppressed[port] = pad
                .as_ref()
                .map_or(0, |pad| port_mask(&self.bindings, pad, self.analog_sticks));
        }
    }

    /// Handles `GamepadAdded`. SDL also emits it for pads present at startup.
    pub fn on_added(&mut self, id: JoystickId) {
        if self.port_of(id).is_some() {
            return;
        }
        let Some(slot) = self.ports.iter().position(Option::is_none) else {
            tracing::info!(?id, "gamepad ignored: all ports in use");
            return;
        };
        match self.subsystem.open(id) {
            Ok(pad) => {
                tracing::info!(port = slot, name = ?pad.name(), "gamepad connected");
                self.ports[slot] = Some(pad);
                self.triggers[slot] = [false; 2];
            }
            Err(err) => tracing::warn!(?id, %err, "cannot open gamepad"),
        }
    }

    /// Handles `GamepadRemoved`; frees the port for the next connected pad.
    pub fn on_removed(&mut self, id: JoystickId) {
        if let Some(slot) = self.port_of(id) {
            tracing::info!(port = slot, "gamepad disconnected");
            self.ports[slot] = None;
        }
    }

    fn port_of(&self, id: JoystickId) -> Option<usize> {
        self.ports
            .iter()
            .position(|pad| pad.as_ref().and_then(|p| p.id().ok()) == Some(id))
    }

    /// A pad input bound to the hold `hotkey` is held on any connected pad.
    pub fn hotkey_held(&self, hotkey: RunnerHotkey) -> bool {
        self.ports.iter().flatten().any(|pad| {
            self.bindings
                .pad_hotkey_held(hotkey, |input| input_held(pad, input))
        })
    }

    /// ORs every connected pad's buttons (and the left stick as D-pad and / or the
    /// raw stick axes, per stick mode) into `snapshot`, minus suppressed buttons that
    /// are still held.
    pub fn read_into(&mut self, snapshot: &mut JoypadStateSnapshot) {
        if self.analog_triggers {
            snapshot.enable_analog_triggers();
        }
        for (port, pad) in self.ports.iter().enumerate() {
            let Some(pad) = pad else { continue };
            let mask = port_mask(&self.bindings, pad, self.analog_sticks);
            let suppressed = &mut self.suppressed[port];
            *suppressed = unsuppress_released(*suppressed, mask);
            snapshot.press_mask(port, mask & !*suppressed);
            snapshot.hold_sticks(
                port,
                self.bindings.pad_sticks(|input| input_held(pad, input)),
            );
            let axes = [Axis::LeftX, Axis::LeftY, Axis::RightX, Axis::RightY];
            if let Some(raw) = raw_stick_axes(self.analog_sticks, axes.map(|a| pad.axis(a))) {
                snapshot.set_axes(port, raw);
            }
            if self.analog_triggers {
                let values = read_pad_triggers(&self.bindings, pad, *suppressed);
                snapshot.raise_triggers(port, values);
            }
        }
    }
}

fn input_held(pad: &Gamepad, input: GamepadInput) -> bool {
    input.is_held(|button| pad.button(button), |axis| pad.axis(axis))
}

/// RetroPad buttons held on `pad` (the left stick counts as D-pad unless it is raw).
fn port_mask(bindings: &InputBindings, pad: &Gamepad, sticks: AnalogSticks) -> u16 {
    let dpad = left_stick_dpad(sticks, pad.axis(Axis::LeftX), pad.axis(Axis::LeftY));
    dpad | bindings.pad_mask(|input| input_held(pad, input))
}

/// Suppressed buttons stay suppressed only while still held.
fn unsuppress_released(suppressed: u16, held: u16) -> u16 {
    suppressed & held
}

#[cfg(test)]
#[path = "sdl_gamepad_manager_tests.rs"]
mod tests;
