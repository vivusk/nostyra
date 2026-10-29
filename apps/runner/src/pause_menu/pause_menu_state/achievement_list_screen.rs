//! "Achievements" screen of the menu state machine: content comes from the loop
//! (`set_achievement_page`), scrolling lives in `AchievementListPageState`; Back
//! returns to the root. Shown on the root only while an achievement session runs.

use super::super::achievement_list_page::AchievementListPage;
use super::super::achievement_list_page_state::{AchievementListPageState, ListNav};
use super::super::menu_types::{MenuAction, NavEvent, Screen};
use super::PauseMenuState;

impl PauseMenuState {
    /// Page content (`None`: no session, the root item disappears).
    pub fn set_achievement_page(&mut self, page: Option<AchievementListPage>) {
        let had_page = self.achievements.available();
        self.achievements.set_page(page);
        if had_page && !self.achievements.available() && self.screen == Screen::Achievements {
            self.screen = Screen::Root;
        }
        self.root = self.root.min(self.root_items().len().saturating_sub(1));
    }

    pub fn achievement_list(&self) -> &AchievementListPageState {
        &self.achievements
    }

    pub(super) fn handle_achievements(&mut self, nav: NavEvent) -> Option<MenuAction> {
        if self.achievements.handle(nav) == ListNav::Leave {
            self.screen = Screen::Root;
        }
        None
    }
}
