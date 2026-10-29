//! "Disc" screen of a multi-disc game: one row per disc (the inserted one marked),
//! Confirm inserts the selected disc (the loop swaps, then resumes), Back returns
//! to the root. Single-disc games never show the root item.

use super::super::menu_types::{
    cycle, root_items, DiscList, MenuAction, NavEvent, RootItem, Screen,
};
use super::PauseMenuState;

impl PauseMenuState {
    /// Discs of the running game (set when the menu opens).
    pub fn set_discs(&mut self, discs: DiscList) {
        self.discs = discs;
    }
    pub fn discs(&self) -> &DiscList {
        &self.discs
    }
    pub fn disc_cursor(&self) -> usize {
        self.disc
    }

    /// Root rows for this game ("Disc" only with more than one disc, "Achievements"
    /// only with an achievement session).
    pub fn root_items(&self) -> Vec<RootItem> {
        root_items(self.discs.is_multi_disc(), self.achievements.available())
    }

    pub(super) fn open_discs(&mut self) {
        self.disc = self
            .discs
            .current
            .min(self.discs.labels.len().saturating_sub(1));
        self.screen = Screen::Discs;
    }

    pub(super) fn handle_discs(&mut self, nav: NavEvent) -> Option<MenuAction> {
        match nav {
            NavEvent::Up | NavEvent::Down => {
                self.disc = cycle(self.disc, self.discs.labels.len(), nav == NavEvent::Down);
                None
            }
            NavEvent::Back => {
                self.screen = Screen::Root;
                None
            }
            NavEvent::Confirm if self.disc == self.discs.current => {
                self.message = Some("Already inserted".to_owned());
                None
            }
            NavEvent::Confirm => {
                self.screen = Screen::Root;
                Some(MenuAction::SwapDisc(self.disc as u32))
            }
            NavEvent::Left | NavEvent::Right => None,
        }
    }
}
