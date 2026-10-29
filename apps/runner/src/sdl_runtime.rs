//! SDL3 for audio + gamepads; winit owns the game window and the OS event loop.
//!
//! Software cores never initialise the SDL video subsystem. OpenGL cores do (lazily,
//! in `hw_render_setup`): it owns one hidden 1×1 window holding their GL context,
//! which is never shown or swapped. Either way SDL never runs the Windows message
//! loop (`SDL_WINDOWS_ENABLE_MESSAGELOOP=0`): winit's loop dispatches every message
//! of this thread, the hidden window's included. [`SdlRuntime::pump`] still pumps
//! SDL's own event queue (gamepad hotplug, buttons/axes, SIGINT quit) on the main
//! thread before each emulated frame.

use input_mapping::JoypadInputSource;
use sdl3::event::Event;
use sdl3::{AudioSubsystem, EventPump, GamepadSubsystem, Sdl};

/// SDL hints for the whole process; set before any SDL init (the GL provider may
/// initialise SDL during `CoreSession::load`, before [`SdlRuntime::init`]).
/// Idempotent.
pub fn set_process_hints() {
    sdl3::hint::set("SDL_WINDOWS_ENABLE_MESSAGELOOP", "0");
    // SDL's only window (GL cores) is hidden, so SDL can't know our focus; the
    // runner decides.
    sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
}

pub struct SdlRuntime {
    _sdl: Sdl,
    audio: Option<AudioSubsystem>,
    event_pump: EventPump,
}

impl SdlRuntime {
    /// Initialises SDL. Gamepads are required; audio is optional (no device on
    /// CI or a broken driver → runner falls back to wall-clock pacing).
    pub fn init() -> anyhow::Result<(Self, GamepadSubsystem)> {
        set_process_hints();
        let sdl = sdl3::init()?;
        let gamepad = sdl.gamepad()?;
        let audio = sdl
            .audio()
            .inspect_err(|err| tracing::warn!(%err, "SDL audio unavailable; running without sound"))
            .ok();
        let event_pump = sdl.event_pump()?;
        Ok((
            Self {
                _sdl: sdl,
                audio,
                event_pump,
            },
            gamepad,
        ))
    }

    pub fn audio(&self) -> Option<&AudioSubsystem> {
        self.audio.as_ref()
    }

    /// Drains SDL events into `input` (hotplug, button and trigger events for the
    /// pause menu and pad hotkeys).
    /// Returns true if SDL asked to quit.
    pub fn pump(&mut self, input: &mut JoypadInputSource) -> bool {
        let mut quit = false;
        for event in self.event_pump.poll_iter() {
            match event {
                Event::GamepadAdded { which, .. } => input.gamepads_mut().on_added(which),
                Event::GamepadRemoved { which, .. } => input.gamepads_mut().on_removed(which),
                Event::GamepadButtonDown { button, .. } => {
                    input.gamepads_mut().on_button(button, true)
                }
                Event::GamepadButtonUp { button, .. } => {
                    input.gamepads_mut().on_button(button, false)
                }
                Event::GamepadAxisMotion {
                    which, axis, value, ..
                } => input.gamepads_mut().on_axis(which, axis, value),
                Event::Quit { .. } => quit = true,
                _ => {}
            }
        }
        quit
    }
}
