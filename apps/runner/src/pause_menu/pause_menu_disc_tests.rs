//! "Disc" root item of multi-disc games: hidden with one disc, listed with two or
//! more, current disc marked, Confirm on another disc asks the loop to swap; the
//! disc screen paints at every size and fits the panel.

use video_renderer::OverlayCanvas;

use super::menu_types::{root_items, DiscList, MenuAction, NavEvent, RootItem, Screen};
use super::pause_menu_state_tests::{menu, press};
use super::pause_menu_view::paint_menu;
use super::slot_thumbnail_cache::SlotThumbnailCache;
use super::SLOT_COUNT;
use NavEvent::{Back, Confirm, Down, Up};

fn discs(count: usize, current: usize) -> DiscList {
    DiscList {
        labels: (1..=count).map(|n| format!("Game (Disc {n})")).collect(),
        current,
    }
}

fn disc_index() -> usize {
    root_items(true, false)
        .iter()
        .position(|i| *i == RootItem::Disc)
        .unwrap_or(usize::MAX)
}

#[test]
fn disc_item_only_for_multi_disc_games() {
    let mut single = menu();
    single.set_discs(discs(1, 0));
    assert!(!single.root_items().contains(&RootItem::Disc));
    assert_eq!(single.root_items(), root_items(false, false));
    let mut multi = menu();
    multi.set_discs(discs(2, 0));
    assert_eq!(multi.root_items(), root_items(true, false));
    // Both root lists keep the shared order around the extra item.
    let with = root_items(true, false);
    assert_eq!(with[disc_index() - 1], RootItem::LoadState);
    assert_eq!(with[disc_index() + 1], RootItem::ResetCore);
    assert_eq!(with.len(), root_items(false, false).len() + 1);
}

#[test]
fn disc_screen_marks_current_and_dispatches_swap() {
    let mut m = menu();
    m.set_discs(discs(3, 1));
    let mut open = vec![Down; disc_index()];
    open.push(Confirm);
    assert_eq!(press(&mut m, &open), None);
    assert_eq!(
        (m.screen(), m.disc_cursor()),
        (Screen::Discs, 1),
        "starts on the inserted disc"
    );
    assert_eq!(
        m.handle(Confirm),
        None,
        "the inserted disc is not swapped again"
    );
    assert_eq!(m.message(), Some("Already inserted"));
    assert_eq!(press(&mut m, &[Down, Down]), None);
    assert_eq!(m.disc_cursor(), 0, "wraps");
    assert_eq!(m.handle(Confirm), Some(MenuAction::SwapDisc(0)));
    assert_eq!(m.screen(), Screen::Root);
    assert_eq!(press(&mut m, &open), None);
    assert_eq!(press(&mut m, &[Up, Back]), None);
    assert_eq!(m.screen(), Screen::Root, "Back returns to the root");
}

#[test]
fn disc_screen_paints_at_every_size() {
    let mut m = menu();
    m.set_discs(discs(8, 7)); // the core's maximum
    let mut open = vec![Down; disc_index()];
    open.push(Confirm);
    press(&mut m, &open);
    let mut thumbs = SlotThumbnailCache::new(vec![std::path::PathBuf::new(); SLOT_COUNT]);
    for size in [(896, 672), (1920, 1080), (300, 200), (1, 1)] {
        let mut canvas = OverlayCanvas::default();
        let text = &mut video_renderer::TtfText::default();
        paint_menu(&mut canvas, (size, "Game"), &m, &mut thumbs, (text, None));
    }
}
