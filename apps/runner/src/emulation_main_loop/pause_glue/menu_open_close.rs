//! Opening and closing the pause menu (split from `pause_glue.rs`). Local play:
//! open drops queued audio and shows "Paused"; close resets the pacer, re-prefills
//! audio and hides pad buttons still held. Netplay (F5): neither — the game keeps
//! running under the menu, which shows the netplay rows instead.

use std::time::Instant;

use ipc_protocol::StateSlotId;

use super::super::EmulationApp;
use super::{format_saved_at, PAUSED_TITLE};
use crate::netplay_gate::menu_mode::menu_transition;
use crate::pause_menu::{DiscList, OptionValues, SlotSummary, SlotThumbnailCache, SLOT_COUNT};

impl EmulationApp<'_> {
    /// Esc, a bound `pause_menu` key / pad input, Guide or the chord: pause and show the root screen.
    pub(in crate::emulation_main_loop) fn open_menu(&mut self) {
        if self.menu.is_open() {
            return;
        }
        if let Some(refused) = self.hardcore_pause_refusal() {
            self.status.show(refused, Instant::now());
            return;
        }
        self.input.release_keyboard();
        self.release_pointer();
        let netplay = self.netplay_menu_info();
        let transition = menu_transition(netplay.is_some());
        if transition.reset_pacing {
            self.timing.resume_normal_speed(); // drops queued audio now; prefill on close
        }
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
        if transition.paused {
            self.set_title_status(PAUSED_TITLE);
        }
        self.menu.set_netplay(netplay); // netplay rows; the game runs on
        tracing::debug!("pause menu opened");
    }

    /// F2 / `select_slot`: pause on the save-state grid to choose the active slot
    /// (arrows + Enter pick it and resume, Esc cancels).
    pub(in crate::emulation_main_loop) fn open_slot_picker(&mut self) {
        self.open_menu();
        self.menu.open_slot_picker();
    }

    /// Resume: hide the menu and restart pacing and audio cleanly.
    pub(in crate::emulation_main_loop) fn close_menu(&mut self) {
        self.menu.close();
        let transition = menu_transition(self.netplay_active());
        if transition.swallow_held_buttons {
            self.input.gamepads_mut().suppress_held_buttons();
        }
        if transition.reset_pacing {
            self.timing.resume_normal_speed();
        }
        if transition.paused {
            self.set_title_status("");
        }
        tracing::debug!("pause menu closed");
    }

    pub(super) fn slot_summary(&self, slot: u8) -> SlotSummary {
        SlotSummary {
            used: self.saves.has_state(StateSlotId::Numbered(slot)),
            saved_at: self.saves.saved_at(slot).and_then(format_saved_at),
        }
    }

    /// Disc labels + inserted disc of a multi-disc game (empty list otherwise).
    pub(super) fn disc_list(&mut self) -> DiscList {
        let Some(discs) = self.session.discs() else {
            return DiscList::default();
        };
        DiscList {
            labels: (0..discs.count()).map(|i| discs.label(i)).collect(),
            current: discs.index() as usize,
        }
    }

    /// Options-screen values from the live config.
    pub(super) fn option_values(&self) -> OptionValues {
        OptionValues::from_config(
            &self.config,
            self.run_ahead.unavailable_reason(),
            self.rewind.unavailable_reason(),
        )
    }

    pub(super) fn set_title_status(&self, status: &str) {
        if let Some(window) = &self.window {
            window.set_title(&self.saves.window_title(status));
        }
    }
}
