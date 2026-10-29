//! State/window press hotkeys from keyboard or pad (bound in config; defaults F1
//! save, F2/F3 slot, F4 load, F11 fullscreen, F10 pause menu, plus fixed Esc):
//! the result goes to the window title and, for state hotkeys, on screen for a few
//! seconds ("Slot 3 · Saved", bottom-left). A successful load clears the rewind
//! history (it belongs to the timeline that was left). Also applies `--load-state`
//! at startup, reported the same way (the `auto` resume point without a slot number).
//! P (default) toggles turbo A for player 1 ("Turbo A: On/Off" on screen).

use std::time::Instant;

use input_mapping::RunnerHotkey;
use ipc_protocol::StateSlotId;

use super::{fast_forward, EmulationApp};
use crate::game_save_data::STATE_LOADED;
use crate::runner_window::toggle_fullscreen;

impl EmulationApp<'_> {
    pub(super) fn on_hotkey(&mut self, hotkey: RunnerHotkey) {
        let status = match hotkey {
            RunnerHotkey::PauseMenu => return self.open_menu(),
            RunnerHotkey::ToggleFullscreen => {
                if let Some(window) = &self.window {
                    toggle_fullscreen(window);
                }
                return;
            }
            // `self.frame` = clean core frame (no OSD/shader) for the thumbnail.
            RunnerHotkey::SaveState => {
                let progress = self.achievement_progress();
                self.saves
                    .save_state(&mut self.session, &self.frame, progress)
            }
            RunnerHotkey::LoadState => {
                let (status, progress) = self.saves.load_state(&mut self.session);
                if status == STATE_LOADED {
                    self.rewind.clear();
                    self.apply_state_progress(progress);
                }
                status
            }
            RunnerHotkey::PrevSlot => self.saves.change_slot(-1),
            RunnerHotkey::NextSlot => self.saves.change_slot(1),
            RunnerHotkey::TurboAToggle => {
                let on = self.input.turbo_a_mut().toggle();
                tracing::info!(on, "turbo A");
                let status = if on { "Turbo A: On" } else { "Turbo A: Off" };
                return self.show_status(status.to_string(), status.to_string());
            }
            // Level-triggered: polled each redraw (`fast_forward_held` / `rewind_held`).
            RunnerHotkey::FastForwardHold | RunnerHotkey::RewindHold => return,
        };
        self.show_state_status(status);
    }

    /// `--load-state`: one attempt per redraw, the first before any frame runs, so a
    /// core that accepts states right away never presents a power-on frame. Retries
    /// (core not ready) stop after `MAX_RETRY_FRAMES` attempts.
    pub(super) fn apply_startup_load(&mut self) {
        let Some(load) = self.startup_load.as_mut() else {
            return;
        };
        let Some(outcome) = load.attempt(&self.saves, &mut self.session) else {
            return;
        };
        let resume_point = load.slot() == StateSlotId::Auto;
        self.startup_load = None;
        if outcome.loaded {
            self.rewind.clear();
            self.apply_state_progress(outcome.progress);
        }
        if resume_point {
            // "Resumed" / "Could not resume": no slot number (F1-F4 slots untouched).
            self.show_status(outcome.status.clone(), outcome.status);
        } else {
            self.show_state_status(outcome.status);
        }
    }

    /// "Slot N · <status>" on screen and in the window title.
    pub(super) fn show_state_status(&mut self, status: String) {
        let osd = format!("Slot {} · {status}", self.saves.slot());
        self.show_status(osd, status);
    }

    /// `osd` on screen, `status` in the window title.
    fn show_status(&mut self, osd: String, status: String) {
        self.status.show(osd, Instant::now());
        if let Some(window) = &self.window {
            let status = if self.fast_forwarding {
                format!("{status} | {}", fast_forward::FAST_FORWARD_STATUS)
            } else {
                status
            };
            window.set_title(&self.saves.window_title(&status));
        }
    }
}
