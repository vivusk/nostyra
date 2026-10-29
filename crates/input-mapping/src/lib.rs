//! Input mapping: SDL3 gamepads + winit keyboard → `RETRO_DEVICE_JOYPAD` and
//! `RETRO_DEVICE_ANALOG` state (plus the touch-screen `RETRO_DEVICE_POINTER` state
//! the runner sets from the mouse), runner hotkeys (press and hold) and pad input
//! events / the Select+Start chord for the pause menu. What every key and pad
//! input does comes from the resolved config (`config_layers::InputConfig` →
//! [`InputBindings`]); this crate only converts config names to winit / SDL values.

#![deny(unsafe_code)]

mod analog_stick_input;
mod analog_stick_mode;
mod analog_trigger_input;
mod game_action_bits;
mod gamepad_input_names;
mod input_bindings;
mod joypad_button;
mod joypad_input_source;
mod joypad_state_snapshot;
mod key_names;
mod keyboard_state;
mod pointer_state;
mod runner_hotkeys;
mod sdl_gamepad_manager;
mod stick_dpad;
mod turbo_button;

pub use analog_stick_input::{StickDirection, ANALOG_MAX};
pub use analog_stick_mode::AnalogSticks;
pub use analog_trigger_input::{AnalogInputs, ANALOG_BUTTON_INDEX, TRIGGER_MAX};
pub use gamepad_input_names::{trigger_held, GamepadInput};
pub use input_bindings::InputBindings;
pub use joypad_input_source::JoypadInputSource;
pub use joypad_state_snapshot::{JoypadButton, JoypadStateSnapshot, PORT_COUNT};
pub use key_names::{key_code_from_name, key_code_name};
pub use pointer_state::{fraction_to_pointer, PointerState, POINTER_MAX};
pub use runner_hotkeys::RunnerHotkey;
pub use sdl_gamepad_manager::SdlGamepadManager;
pub use stick_dpad::{stick_to_dpad, STICK_DEADZONE};
pub use turbo_button::{TurboButton, TURBO_HALF_PERIOD_FRAMES};
