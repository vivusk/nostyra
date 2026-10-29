//! Pause menu ↔ loop. While the menu is open the game is paused: no `retro_run`
//! (so no rewind captures, run-ahead or SRAM ticks), queued audio is dropped at
//! once (silence, never a looping buffer), game input is released, and the last
//! frame keeps presenting (with its shader) under the dimmed menu overlay. Closing
//! resets the frame pacer (no catch-up for the paused time), re-prefills audio and
//! hides pad buttons still held (Confirm) from the core until released; held keys
//! are swallowed by the menu input adapter. Play time keeps counting while paused
//! (M1 session semantics). In achievement hardcore a pause is granted only when
//! rc_client allows it (`rc_client_can_pause`); otherwise the game keeps running
//! with "Pause available in N s".

use std::time::{Duration, Instant};

use input_mapping::RunnerHotkey;
use ipc_protocol::StateSlotId;
use winit::event_loop::ActiveEventLoop;

use super::EmulationApp;
use crate::pause_menu::{
    DiscList, NavEvent, OptionValues, PadRoute, SlotSummary, SlotThumbnailCache, SLOT_COUNT,
};

mod menu_actions;

/// On-screen hint when the game window opens (Esc no longer quits).
pub(super) const MENU_HINT: &str = "Esc: menu";
/// Paused redraws need no more than ~120 Hz; without this an `Immediate` present
/// mode (audio-sync) would spin, since no audio wait paces the loop while paused.
const PAUSED_REDRAW_SLEEP: Duration = Duration::from_millis(8);
const PAUSED_TITLE: &str = "Paused";

impl EmulationApp<'_> {
    /// Once per redraw after the SDL pump: pad inputs (Guide, navigation, pad
    /// hotkeys on a fresh press while the menu is closed), the Select+Start chord and held-direction repeats. Returns whether the game is
    /// paused (menu open: no frames this redraw).
    pub(super) fn update_pause_menu(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let now = Instant::now();
        for (pad_input, pressed) in self.input.gamepads_mut().take_pad_events() {
            let hotkey = self.input.bindings().pad_hotkey(pad_input);
            let menu_binding = hotkey == Some(RunnerHotkey::PauseMenu);
            match self
                .menu
                .on_pad_input(pad_input, pressed, menu_binding, now)
            {
                PadRoute::Open => self.open_menu(),
                PadRoute::Nav(nav) => self.menu_nav(event_loop, nav),
                PadRoute::Pass => {
                    if let Some(hotkey) = hotkey {
                        self.on_hotkey(hotkey);
                    }
                }
                PadRoute::Ignore => {}
            }
        }
        let chord = self.input.gamepads_mut().menu_chord_held();
        if self.menu.poll_chord(chord, now) {
            self.open_menu();
        }
        if let Some(nav) = self.menu.poll_repeat(now) {
            self.menu_nav(event_loop, nav);
        }
        let paused = self.menu.is_open();
        if paused {
            self.achievements_idle();
            std::thread::sleep(PAUSED_REDRAW_SLEEP);
        }
        paused
    }

    /// Esc, a bound `pause_menu` key / pad input, Guide or the chord: pause and show the root screen.
    pub(super) fn open_menu(&mut self) {
        if self.menu.is_open() {
            return;
        }
        if let Some(refused) = self.hardcore_pause_refusal() {
            self.status.show(refused, Instant::now());
            return;
        }
        self.input.release_keyboard();
        self.release_pointer();
        self.timing.resume_normal_speed(); // drops queued audio now; prefill on close
        let slots = (0..SLOT_COUNT as u8)
            .map(|slot| self.slot_summary(slot))
            .collect();
        let thumbnails = SlotThumbnailCache::new(
            (0..SLOT_COUNT as u8)
                .map(|slot| self.saves.thumbnail_path(slot))
                .collect(),
        );
        let discs = self.disc_list();
        self.menu.open(
            slots,
            self.option_values(),
            self.saves.slot(),
            thumbnails,
            discs,
        );
        self.refresh_achievement_list(true);
        self.set_title_status(PAUSED_TITLE);
        tracing::debug!("pause menu opened");
    }

    /// Resume: hide the menu and restart pacing and audio cleanly.
    pub(super) fn close_menu(&mut self) {
        self.menu.close();
        self.input.gamepads_mut().suppress_held_buttons();
        self.timing.resume_normal_speed();
        self.set_title_status("");
        tracing::debug!("pause menu closed");
    }

    /// One navigation input for the open menu (keyboard via `route_key`, pad here).
    pub(super) fn menu_nav(&mut self, event_loop: &ActiveEventLoop, nav: NavEvent) {
        if let Some(action) = self.menu.handle(nav) {
            self.apply_menu_action(event_loop, action);
        }
    }

    fn slot_summary(&self, slot: u8) -> SlotSummary {
        SlotSummary {
            used: self.saves.has_state(StateSlotId::Numbered(slot)),
            saved_at: self.saves.saved_at(slot).and_then(format_saved_at),
        }
    }

    /// Disc labels + inserted disc of a multi-disc game (empty list otherwise).
    fn disc_list(&mut self) -> DiscList {
        let Some(discs) = self.session.discs() else {
            return DiscList::default();
        };
        DiscList {
            labels: (0..discs.count()).map(|i| discs.label(i)).collect(),
            current: discs.index() as usize,
        }
    }

    /// Options-screen values from the live config.
    fn option_values(&self) -> OptionValues {
        OptionValues::from_config(
            &self.config,
            self.run_ahead.unavailable_reason(),
            self.rewind.unavailable_reason(),
        )
    }

    fn set_title_status(&self, status: &str) {
        if let Some(window) = &self.window {
            window.set_title(&self.saves.window_title(status));
        }
    }
}

/// Unix seconds → local "2026-10-18 14:02".
fn format_saved_at(unix_secs: u64) -> Option<String> {
    let utc = chrono::DateTime::from_timestamp(i64::try_from(unix_secs).ok()?, 0)?;
    let local = utc.with_timezone(&chrono::Local);
    Some(local.format("%Y-%m-%d %H:%M").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_time_is_minute_precision_local_time() {
        let text = format_saved_at(1_792_300_000).unwrap_or_default();
        assert_eq!(text.len(), "2026-10-18 14:02".len(), "{text}");
        assert!(text.starts_with("2026-"), "{text}");
        assert_eq!(format_saved_at(u64::MAX), None);
    }
}
