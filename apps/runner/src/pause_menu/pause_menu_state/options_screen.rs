//! "Options" screen: Up / Down pick a row, Left / Right / Confirm change its value
//! (or run its save / reset action), Back returns to the root.

use super::super::menu_option_values::OPTION_ITEMS;
use super::super::menu_types::{cycle, MenuAction, NavEvent, Screen};
use super::PauseMenuState;

impl PauseMenuState {
    pub(super) fn handle_options(&mut self, nav: NavEvent) -> Option<MenuAction> {
        let forward = match nav {
            NavEvent::Up | NavEvent::Down => {
                self.option = cycle(self.option, OPTION_ITEMS.len(), nav == NavEvent::Down);
                return None;
            }
            NavEvent::Back => {
                self.screen = Screen::Root;
                return None;
            }
            NavEvent::Left => false,
            NavEvent::Right | NavEvent::Confirm => true,
        };
        let item = OPTION_ITEMS[self.option];
        self.options.adjust(item, forward, nav == NavEvent::Confirm)
    }
}
