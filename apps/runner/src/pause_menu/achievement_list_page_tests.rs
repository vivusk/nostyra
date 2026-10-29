//! "Achievements" in the pause menu: root item only with a session, open / scroll /
//! back through the state machine, the page paints dimmed at every size with badges.

use video_renderer::{OverlayCanvas, TtfText};

use super::achievement_list_page::{AchievementListPage, ListEntry, ListHeader, ListRow};
use super::menu_types::{NavEvent, RootItem, Screen, SLOT_COUNT};
use super::pause_menu_state_tests::{menu, press};
use super::pause_menu_view::paint_menu;
use super::slot_thumbnail_cache::SlotThumbnailCache;
use crate::achievement_overlay::{BadgeCache, BadgeKey};
use NavEvent::{Back, Confirm, Down};

pub(crate) fn sample_page(rows: usize) -> AchievementListPage {
    let mut entries = vec![ListEntry::Section("Locked".into())];
    entries.extend((0..rows).map(|i| {
        ListEntry::Row(ListRow {
            badge: Some(BadgeKey::Achievement {
                name: ["00101", "00102"][i % 2].into(),
                locked: i % 3 != 0,
            }),
            title: format!("Tầng {i}: Pokémon Привет"),
            description: "Collect every coin in the first world without losing a life".into(),
            points: 10,
            progress: if i == 1 { "5/10".into() } else { String::new() },
            unlocked: i % 3 == 0,
        })
    }));
    AchievementListPage {
        header: ListHeader {
            icon: Some(BadgeKey::GameIcon("085573".into())),
            title: "Test Game".into(),
            summary: "Softcore · 0 of 12 unlocked · 0 of 120 points".into(),
        },
        entries,
    }
}

fn open_achievements(m: &mut super::pause_menu_state::PauseMenuState) {
    let index = m
        .root_items()
        .iter()
        .position(|i| *i == RootItem::Achievements);
    let mut navs = vec![Down; index.expect("achievements item")];
    navs.push(Confirm);
    press(m, &navs);
}

#[test]
fn achievements_item_appears_only_with_a_session_and_backs_out() {
    let mut m = menu();
    assert!(!m.root_items().contains(&RootItem::Achievements));
    m.set_achievement_page(Some(sample_page(12)));
    let items = m.root_items();
    let at = items
        .iter()
        .position(|i| *i == RootItem::Achievements)
        .unwrap();
    assert_eq!(
        items[at + 1],
        RootItem::ResetCore,
        "after load / disc, before reset"
    );
    open_achievements(&mut m);
    assert_eq!(m.screen(), Screen::Achievements);
    press(&mut m, &[Down, Down]);
    assert_eq!(m.achievement_list().cursor(), 3);
    assert_eq!(press(&mut m, &[Confirm]), None, "rows have no action");
    press(&mut m, &[Back]);
    assert_eq!(m.screen(), Screen::Root);
    press(&mut m, &[Confirm]); // the root cursor stayed on "Achievements"
    assert_eq!(m.screen(), Screen::Achievements);
    m.set_achievement_page(None);
    assert_eq!(m.screen(), Screen::Root, "session gone: back to the root");
    assert!(m.root_cursor() < m.root_items().len());
}

#[test]
fn page_paints_dimmed_with_badges_at_every_size() {
    let data = tempfile::tempdir().unwrap();
    let mut badges = BadgeCache::new(data.path());
    let mut thumbs = SlotThumbnailCache::new(vec![std::path::PathBuf::new(); SLOT_COUNT]);
    let mut text = TtfText::default();
    let mut m = menu();
    m.set_achievement_page(Some(sample_page(40)));
    open_achievements(&mut m);
    for size in [(1280, 720), (1920, 1080), (3840, 2160), (300, 200), (1, 1)] {
        let mut canvas = OverlayCanvas::default();
        let tools = (&mut text, Some(&mut badges));
        paint_menu(&mut canvas, (size, "Game"), &m, &mut thumbs, tools);
        assert!(canvas.pixels().chunks(4).all(|px| px[3] >= 179), "{size:?}");
    }
    let mut canvas = OverlayCanvas::default();
    paint_menu(
        &mut canvas,
        ((1280, 720), "Game"),
        &m,
        &mut thumbs,
        (&mut text, None),
    );
    let bright = canvas.pixels().chunks(4).filter(|px| px[0] > 200).count();
    assert!(bright > 500, "title / row text drawn ({bright} px)");
    assert!(m.achievement_list().top() == 0);
    let visible = (0..30).fold(0, |_, _| {
        press(&mut m, &[Down]);
        m.achievement_list().top()
    });
    assert!(visible > 0, "scrolled after moving past the visible rows");
}

/// (title, unlocked) of every row, in order (session tests).
pub(crate) fn row_states(page: &AchievementListPage) -> Vec<(String, bool)> {
    page.entries
        .iter()
        .filter_map(|entry| match entry {
            ListEntry::Row(row) => Some((row.title.clone(), row.unlocked)),
            ListEntry::Section(_) => None,
        })
        .collect()
}
