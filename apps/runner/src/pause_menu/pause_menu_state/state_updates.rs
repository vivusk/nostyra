//! Updates the loop pushes into an open menu: a saved slot, new option values, a
//! footer message, the slot picker.

use super::super::menu_option_values::OptionValues;
use super::super::menu_types::{Screen, SlotMode, SlotSummary};
use super::PauseMenuState;

impl PauseMenuState {
    pub fn set_slot(&mut self, slot: u8, summary: SlotSummary) {
        if let Some(cell) = self.slots.get_mut(usize::from(slot)) {
            *cell = summary;
        }
    }

    pub fn set_options(&mut self, options: OptionValues) {
        self.options = options;
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
    }

    /// Shows the grid as the slot picker (cursor on the active slot).
    pub fn open_slot_picker(&mut self) {
        self.screen = Screen::Slots(SlotMode::Select);
    }
}
