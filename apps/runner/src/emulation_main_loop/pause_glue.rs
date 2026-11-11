//! Pause menu ↔ loop. While the menu is open the game is paused: no `retro_run`
//! (so no rewind captures, run-ahead or SRAM ticks), queued audio is dropped at
//! once (silence, never a looping buffer), game input is released, and the last
//! frame keeps presenting (with its shader) under the dimmed menu overlay. Closing
//! resets the frame pacer (no catch-up for the paused time), re-prefills audio and
//! hides pad buttons still held (Confirm) from the core until released; held keys
//! are swallowed by the menu input adapter. Play time keeps counting while paused
//! (M1 session semantics). In achievement hardcore a pause is granted only when
//! rc_client allows it (`rc_client_can_pause`); otherwise the game keeps running
//! with "Pause available in N s". In netplay the menu never pauses (F5,
//! `menu_open_close`): frames keep ticking, the local port sends neutral input.

use std::time::{Duration, Instant};

use input_mapping::RunnerHotkey;
use winit::event_loop::ActiveEventLoop;

use super::EmulationApp;
use crate::netplay_gate::menu_mode::menu_pauses;
use crate::pause_menu::{NavEvent, PadRoute};

mod menu_actions;
mod menu_open_close;

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
        // Netplay: the menu opens over the running game (F5), lockstep goes on.
        let paused = menu_pauses(self.menu.is_open(), self.netplay_gate().active());
        if paused {
            self.achievements_idle();
            std::thread::sleep(PAUSED_REDRAW_SLEEP);
        }
        paused
    }

    /// One navigation input for the open menu (keyboard via `route_key`, pad here).
    pub(super) fn menu_nav(&mut self, event_loop: &ActiveEventLoop, nav: NavEvent) {
        if let Some(action) = self.menu.handle(nav) {
            self.apply_menu_action(event_loop, action);
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
