//! Scrolling of the achievement page: rows only, sections stay visible above their
//! first row, page steps, content refresh keeps the position.

use super::*;
use crate::pause_menu::achievement_list_page::{ListHeader, ListRow};

fn row(title: &str) -> ListEntry {
    ListEntry::Row(ListRow {
        badge: None,
        title: title.into(),
        description: String::new(),
        points: 5,
        progress: String::new(),
        unlocked: false,
    })
}

/// Section "A" + 3 rows, section "B" + 10 rows (entries 0..15).
fn page() -> AchievementListPage {
    let mut entries = vec![ListEntry::Section("A".into())];
    entries.extend((0..3).map(|i| row(&format!("a{i}"))));
    entries.push(ListEntry::Section("B".into()));
    entries.extend((0..10).map(|i| row(&format!("b{i}"))));
    AchievementListPage {
        header: ListHeader::default(),
        entries,
    }
}

#[test]
fn cursor_skips_sections_and_scrolls_with_the_selection() {
    let mut list = AchievementListPageState::default();
    assert!(!list.available());
    list.set_page(Some(page()));
    list.set_visible(4);
    assert_eq!((list.cursor(), list.top()), (1, 0), "first row selected");
    for _ in 0..3 {
        assert_eq!(list.handle(NavEvent::Down), ListNav::Stay);
    }
    assert_eq!(list.cursor(), 5, "section B skipped");
    assert_eq!(list.top(), 2, "cursor row is the last visible entry");
    list.handle(NavEvent::Right);
    assert_eq!(list.cursor(), 8, "page = visible - 1 rows");
    for _ in 0..20 {
        list.handle(NavEvent::Down);
    }
    assert_eq!(
        (list.cursor(), list.top()),
        (14, 11),
        "stops at the last row"
    );
    list.handle(NavEvent::Left);
    list.handle(NavEvent::Left);
    list.handle(NavEvent::Left);
    list.handle(NavEvent::Up);
    assert_eq!(list.cursor(), 3);
    for _ in 0..5 {
        list.handle(NavEvent::Up);
    }
    assert_eq!(
        (list.cursor(), list.top()),
        (1, 0),
        "section A label back in view"
    );
    assert_eq!(list.handle(NavEvent::Confirm), ListNav::Stay);
    assert_eq!(list.handle(NavEvent::Back), ListNav::Leave);
}

#[test]
fn refresh_keeps_the_position_and_empty_pages_are_safe() {
    let mut list = AchievementListPageState::default();
    list.set_page(Some(page()));
    list.handle(NavEvent::Down);
    list.handle(NavEvent::Down);
    list.set_page(Some(page()));
    assert_eq!(list.cursor(), 3, "same row after an unlock refresh");
    list.set_page(Some(AchievementListPage::default()));
    assert_eq!((list.cursor(), list.top()), (0, 0));
    assert_eq!(
        list.handle(NavEvent::Down),
        ListNav::Stay,
        "no rows: no panic"
    );
    list.set_page(None);
    assert!(!list.available());
}
