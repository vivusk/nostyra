//! winit event handling for the game window. Keyboard input goes through
//! `route_key`: the pause menu first (while open, or to swallow keys still held
//! from it), then game bindings and held fast-forward / rewind keys, then press
//! hotkeys (all from the config bindings; Esc always opens the menu). Mouse cursor
//! and left button feed the touch-screen pointer (`pointer_events`).

use std::time::Instant;

use input_mapping::RunnerHotkey;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId;

use super::EmulationApp;
use crate::pause_menu::KeyRoute;

impl EmulationApp<'_> {
    /// Menu first; otherwise joypad keys go to the core and, on a fresh press (not
    /// a repeat), the remaining keys trigger runner hotkeys.
    fn route_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        key: KeyCode,
        pressed: bool,
        repeat: bool,
    ) {
        let menu_binding = self.input.key_hotkey(key) == Some(RunnerHotkey::PauseMenu);
        match self
            .menu
            .on_key(key, pressed, repeat, menu_binding, Instant::now())
        {
            KeyRoute::Pass => {}
            KeyRoute::Consumed => return,
            KeyRoute::Nav(nav) => return self.menu_nav(event_loop, nav),
        }
        if self.input.on_key(key, pressed) || !pressed || repeat {
            return;
        }
        if let Some(hotkey) = self.input.key_hotkey(key) {
            self.on_hotkey(hotkey);
        }
    }
}

impl ApplicationHandler for EmulationApp<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            match self.create_window(event_loop) {
                Ok(()) => self.report_started(),
                Err(err) => self.fail(event_loop, err),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(presenter) = self.presenter.as_mut() {
                    presenter.resize(size.width, size.height);
                }
            }
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.refresh_sync_mode(false)
            }
            WindowEvent::Focused(false) => {
                self.input.release_keyboard();
                self.menu.release_keys();
                self.release_pointer();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state,
                        repeat,
                        ..
                    },
                ..
            } => self.route_key(event_loop, key, state == ElementState::Pressed, repeat),
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_moved(Some((position.x, position.y)))
            }
            WindowEvent::CursorLeft { .. } => self.pointer_moved(None),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => self.pointer_button(state == ElementState::Pressed),
            WindowEvent::RedrawRequested => match self.tick(event_loop) {
                Err(err) => self.fail(event_loop, err),
                Ok(()) => {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            },
            _ => {}
        }
    }
}
