//! Performs pause-menu actions: slot save / load, live option changes (shader via
//! the async loader, scaling on the presenter, run-ahead and rewind on the loop
//! state; rewind off frees its ring), config layer writes, core reset and quit.
//! Results show in the menu footer; a successful load or a reset resumes the game.

use std::time::Instant;

use config_layers::ResolvedConfig;
use winit::event_loop::ActiveEventLoop;

use super::super::rewind_playback::Rewind;
use super::super::EmulationApp;
use crate::game_save_data::{STATE_LOADED, STATE_SAVED};
use crate::pause_menu::MenuAction;
use crate::runner_effective_config::SaveTarget;
use crate::state_thumbnail_writer::Thumbnail;

impl EmulationApp<'_> {
    pub(super) fn apply_menu_action(&mut self, event_loop: &ActiveEventLoop, action: MenuAction) {
        let mut live = self.config.clone();
        match action {
            MenuAction::Resume => return self.close_menu(),
            // Same path as closing the window: SRAM flushed in `finish`, session reported.
            MenuAction::Quit => return event_loop.exit(),
            MenuAction::SaveSlot(slot) => return self.menu_save_slot(slot),
            MenuAction::LoadSlot(slot) => return self.menu_load_slot(slot),
            MenuAction::SaveForGame => return self.save_config(SaveTarget::Game),
            MenuAction::SaveAsDefault => return self.save_config(SaveTarget::Global),
            MenuAction::ResetGameSettings => return self.reset_game_config(),
            MenuAction::ResetCore => return self.reset_core(),
            MenuAction::SwapDisc(index) => return self.swap_disc(index),
            MenuAction::SetShader(shader) => live.video.shader = shader,
            MenuAction::SetScaling(scaling) => live.video.scaling = scaling,
            MenuAction::SetRunAhead(frames) => live.latency.run_ahead_frames = frames,
            MenuAction::SetRewind(enabled) => live.rewind.enabled = enabled,
        }
        self.apply_live_config(live);
    }

    /// Saves into `slot` (it becomes the active slot, like F2/F3 + F1). The grid
    /// cell and its thumbnail update from memory right away.
    fn menu_save_slot(&mut self, slot: u8) {
        self.saves.select_slot(slot);
        self.set_title_status(super::PAUSED_TITLE); // title shows the new active slot
        let progress = self.achievement_progress();
        let status = self
            .saves
            .save_state(&mut self.session, &self.frame, progress);
        if status == STATE_SAVED {
            let summary = self.slot_summary(slot);
            self.menu
                .set_slot(slot, summary, Thumbnail::from_frame(&self.frame));
        }
        self.menu.set_message(format!("Slot {slot} · {status}"));
    }

    /// Loads `slot`; on success the history of the left timeline is dropped and the
    /// game resumes with the result on screen.
    fn menu_load_slot(&mut self, slot: u8) {
        self.saves.select_slot(slot);
        let (status, progress) = self.saves.load_state(&mut self.session);
        if status == STATE_LOADED {
            self.rewind.clear();
            self.apply_state_progress(progress);
            self.close_menu();
            self.show_state_status(status);
        } else {
            self.menu.set_message(format!("Slot {slot} · {status}"));
        }
    }

    /// "Reset game": soft reset (`retro_reset`) while paused, i.e. between two
    /// `retro_run` calls, which every core handles. SRAM is flushed first so the
    /// battery save on disk is a safety point should the game wipe it after boot;
    /// the rewind history belongs to the run before the reset and is dropped.
    /// Closing the menu drops queued audio and resets pacing (same as Resume).
    /// Auto-save, the resume point and play time are untouched.
    fn reset_core(&mut self) {
        self.saves.flush(&mut self.session);
        self.session.reset();
        self.achievements_reset();
        self.rewind.clear();
        self.close_menu();
        self.status.show("Game reset", Instant::now());
        tracing::info!("core reset from the pause menu");
    }

    /// Inserts another disc (tray open → select → close) and resumes; the core reads
    /// the new disc a moment later. Save states keep the disc index themselves.
    fn swap_disc(&mut self, index: u32) {
        let result = self.session.discs().map(|discs| discs.swap_to(index));
        match result {
            Some(Ok(())) => {
                // Captures taken with the previous disc must not be rewound into.
                self.rewind.clear();
                self.achievements_disc_swapped(index);
                self.close_menu();
                let text = format!("Disc {} inserted", index + 1);
                self.status.show(text, Instant::now());
                tracing::info!(disc = index + 1, "disc swapped from the pause menu");
            }
            Some(Err(err)) => {
                tracing::warn!("disc swap failed: {err}");
                self.menu.set_message("Disc change failed");
            }
            None => self.menu.set_message("Disc change not supported"),
        }
    }

    /// Writes the keys changed in this session to the game or global layer.
    fn save_config(&mut self, target: SaveTarget) {
        let message = match self.config_layers.save_changes(&self.config, target) {
            Ok(0) => "No changes to save",
            Ok(_) if target == SaveTarget::Game => "Saved for this game",
            Ok(_) => "Saved as default",
            Err(err) => {
                tracing::warn!("settings save failed: {err}");
                "Settings save failed"
            }
        };
        self.menu.set_message(message);
    }

    /// Drops this game's overrides and applies what the remaining layers give.
    fn reset_game_config(&mut self) {
        let result = self.config_layers.reset_game_layer();
        self.apply_live_config(self.config_layers.resolved().clone());
        self.menu.set_options(self.option_values());
        self.menu.set_message(match result {
            Ok(()) => "Game settings reset",
            Err(err) => {
                tracing::warn!("settings reset failed: {err}");
                "Settings reset failed"
            }
        });
    }

    /// Makes `live` the session config and applies what changed.
    fn apply_live_config(&mut self, live: ResolvedConfig) {
        let old = std::mem::replace(&mut self.config, live);
        if old.video.shader != self.config.video.shader {
            self.request_configured_shader();
        }
        if old.video.scaling != self.config.video.scaling {
            self.apply_scaling();
        }
        if old.latency != self.config.latency {
            self.run_ahead
                .set_frames(self.config.latency.run_ahead_frames);
        }
        if old.rewind != self.config.rewind {
            self.rewind = Rewind::new(&self.config.rewind, &self.session, self.savestates);
            self.rewind.set_hardcore_gate(self.hardcore_gate());
        }
        tracing::info!(config = ?self.config, "live config changed");
    }
}
