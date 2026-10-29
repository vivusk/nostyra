//! Root-screen questions: "Reset game" (core reset) and "Quit" both ask first,
//! default to No, and only Yes emits their action.

use super::menu_types::{root_items, MenuAction, NavEvent, RootItem, Screen};
use super::pause_menu_state_tests::{menu, press};
use NavEvent::{Back, Confirm, Down, Left, Right, Up};

/// Position of `item` on the root screen (tests never hard-code root indices).
pub(super) fn root_index(item: RootItem) -> usize {
    root_items(false, false)
        .iter()
        .position(|i| *i == item)
        .unwrap_or(usize::MAX)
}

/// Presses from a fresh root cursor that open `item` (Down × index, Confirm).
pub(super) fn open_root(item: RootItem) -> Vec<NavEvent> {
    let mut navs = vec![Down; root_index(item)];
    navs.push(Confirm);
    navs
}

#[test]
fn root_order_puts_reset_game_between_load_and_options() {
    assert_eq!(
        root_index(RootItem::ResetCore),
        root_index(RootItem::LoadState) + 1
    );
    assert_eq!(
        root_index(RootItem::Options),
        root_index(RootItem::ResetCore) + 1
    );
}

#[test]
fn reset_game_asks_first_and_defaults_to_no() {
    let reset = root_index(RootItem::ResetCore);
    let mut m = menu();
    assert_eq!(press(&mut m, &open_root(RootItem::ResetCore)), None);
    let asked = (m.screen(), m.confirm_yes());
    assert_eq!(asked, (Screen::ConfirmReset, false), "No is the default");
    assert_eq!(m.handle(Confirm), None, "No keeps playing");
    assert_eq!((m.screen(), m.root_cursor()), (Screen::Root, reset));
    assert_eq!(press(&mut m, &[Confirm, Right, Back]), None, "Back = No");
    assert_eq!((m.screen(), m.root_cursor()), (Screen::Root, reset));
    assert_eq!(press(&mut m, &[Confirm, Left, Up]), None);
    assert!(!m.confirm_yes(), "asks again on No; directions toggle");
    assert_eq!(press(&mut m, &[Down, Confirm]), Some(MenuAction::ResetCore));
    assert_eq!(m.screen(), Screen::Root, "the loop closes the menu");
}

#[test]
fn quit_asks_first_and_defaults_to_no() {
    let quit = root_index(RootItem::Quit);
    let mut m = menu();
    assert_eq!(press(&mut m, &[Up, Confirm]), None, "Up wraps to Quit");
    let asked = (m.screen(), m.confirm_yes());
    assert_eq!(asked, (Screen::ConfirmQuit, false), "No is the default");
    assert_eq!(m.handle(Confirm), None, "No keeps playing");
    assert_eq!((m.screen(), m.root_cursor()), (Screen::Root, quit));
    assert_eq!(press(&mut m, &[Confirm, Right, Back]), None, "Back cancels");
    assert_eq!((m.screen(), m.root_cursor()), (Screen::Root, quit));
    assert_eq!(press(&mut m, &[Confirm, Left, Up]), None);
    assert!(!m.confirm_yes(), "asks again on No; directions toggle");
    assert_eq!(press(&mut m, &[Down, Confirm]), Some(MenuAction::Quit));
}
