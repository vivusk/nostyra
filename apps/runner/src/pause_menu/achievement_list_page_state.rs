//! Scroll / selection state of the "Achievements" page, apart from the menu state
//! machine: the page content, the selected row (sections are skipped) and the first
//! visible entry. The view reports how many entries fit (`visible`), so scrolling
//! keeps the selection on screen. Navigation: Up / Down one row (held keys repeat),
//! Left / Right one page, Back to the root.

use std::cell::Cell;

use super::achievement_list_page::{AchievementListPage, ListEntry};
use super::menu_types::NavEvent;

/// Entries assumed visible before the first paint.
const DEFAULT_VISIBLE: usize = 6;

/// What a navigation input did on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListNav {
    Stay,
    Leave,
}

#[derive(Default)]
pub struct AchievementListPageState {
    /// `None` without an achievement session (the root item is hidden).
    page: Option<AchievementListPage>,
    /// Entry index of the selected row.
    cursor: usize,
    /// First visible entry.
    top: usize,
    /// Entries that fit on screen (written by the view at paint time).
    visible: Cell<usize>,
}

impl AchievementListPageState {
    pub fn available(&self) -> bool {
        self.page.is_some()
    }

    pub fn page(&self) -> Option<&AchievementListPage> {
        self.page.as_ref()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn top(&self) -> usize {
        self.top
    }

    /// The view tells how many entries fit (at least one).
    pub fn set_visible(&self, entries: usize) {
        self.visible.set(entries.max(1));
    }

    /// New content (menu opened, unlock or set load while open): the selection stays
    /// on the same position when it still exists.
    pub fn set_page(&mut self, page: Option<AchievementListPage>) {
        self.page = page;
        let rows = self.rows();
        let cursor = rows.iter().copied().find(|&i| i >= self.cursor);
        self.cursor = cursor.or(rows.last().copied()).unwrap_or(0);
        self.scroll_to_cursor();
    }

    /// Applies one input; [`ListNav::Leave`] = back to the root screen.
    pub fn handle(&mut self, nav: NavEvent) -> ListNav {
        let rows = self.rows();
        let position = rows.iter().position(|&i| i == self.cursor).unwrap_or(0);
        let page = self.visible().saturating_sub(1).max(1);
        let target = match nav {
            NavEvent::Back => return ListNav::Leave,
            NavEvent::Confirm => return ListNav::Stay,
            NavEvent::Up => position.saturating_sub(1),
            NavEvent::Down => position + 1,
            NavEvent::Left => position.saturating_sub(page),
            NavEvent::Right => position + page,
        };
        if let Some(&entry) = rows.get(target.min(rows.len().saturating_sub(1))) {
            self.cursor = entry;
        }
        self.scroll_to_cursor();
        ListNav::Stay
    }

    fn visible(&self) -> usize {
        match self.visible.get() {
            0 => DEFAULT_VISIBLE,
            n => n,
        }
    }

    fn rows(&self) -> Vec<usize> {
        self.page
            .as_ref()
            .map(AchievementListPage::row_indexes)
            .unwrap_or_default()
    }

    /// Keeps the cursor row (and the section label just above it) on screen.
    fn scroll_to_cursor(&mut self) {
        let above_is_section = self.cursor > 0
            && self.page.as_ref().is_some_and(|page| {
                matches!(
                    page.entries.get(self.cursor - 1),
                    Some(ListEntry::Section(_))
                )
            });
        let first_wanted = self.cursor - usize::from(above_is_section);
        if first_wanted < self.top {
            self.top = first_wanted;
        }
        let visible = self.visible();
        if self.cursor >= self.top + visible {
            self.top = self.cursor + 1 - visible;
        }
    }
}

#[cfg(test)]
#[path = "achievement_list_page_state_tests.rs"]
mod tests;
