//! Yes / No questions of the pause menu (overwrite a used slot, reset the game,
//! quit the game): "No" is the default, so a stray Confirm never destroys, resets
//! or exits anything.

use super::super::menu_types::{MenuAction, NavEvent, Screen, SlotMode};
use super::PauseMenuState;

impl PauseMenuState {
    /// Shows a Yes / No question with "No" highlighted.
    pub(super) fn ask(&mut self, question: Screen) {
        self.screen = question;
        self.confirm_yes = false;
    }

    /// Directions toggle Yes / No, Confirm answers, Back = No; both go back to the
    /// screen that asked.
    pub(super) fn handle_confirm(&mut self, nav: NavEvent) -> Option<MenuAction> {
        let (parent, action) = match self.screen {
            Screen::ConfirmOverwrite(slot) => {
                (Screen::Slots(SlotMode::Save), MenuAction::SaveSlot(slot))
            }
            Screen::ConfirmReset => (Screen::Root, MenuAction::ResetCore),
            Screen::ConfirmQuit => (Screen::Root, MenuAction::Quit),
            // Not a question: nothing to answer (callers only route questions here).
            _ => return None,
        };
        match nav {
            NavEvent::Confirm => {
                self.screen = parent;
                return self.confirm_yes.then_some(action);
            }
            NavEvent::Back => self.screen = parent,
            _ => self.confirm_yes = !self.confirm_yes,
        }
        None
    }
}
