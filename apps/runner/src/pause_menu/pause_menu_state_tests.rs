//! State-machine tests: navigation, disabled items, emitted actions (root
//! questions in `pause_menu_root_tests`).

use config_layers::{ResolvedConfig, VideoScaling};

use super::menu_option_values::{OptionItem, OptionValues, OPTION_ITEMS};
use super::menu_types::{
    MenuAction, NavEvent, RootItem, Screen, SlotMode, SlotSummary, SLOT_COUNT,
};
use super::pause_menu_root_tests::{open_root, root_index};
use super::pause_menu_state::PauseMenuState;
use NavEvent::{Back, Confirm, Down, Left, Right, Up};

/// Slots 1 and 7 hold states; the active slot is 0.
pub(super) fn menu() -> PauseMenuState {
    let slots = (0..SLOT_COUNT)
        .map(|n| SlotSummary {
            used: n == 1 || n == 7,
            saved_at: Some("2026-10-18 14:02".into()),
        })
        .collect();
    let options = OptionValues::from_config(&ResolvedConfig::default(), None, None);
    PauseMenuState::new(slots, options, 0)
}

pub(super) fn press(menu: &mut PauseMenuState, navs: &[NavEvent]) -> Option<MenuAction> {
    navs.iter().fold(None, |_, nav| menu.handle(*nav))
}

#[test]
fn back_on_root_resumes_and_root_items_act() {
    let mut m = menu();
    assert_eq!(m.handle(Back), Some(MenuAction::Resume));
    assert_eq!(m.handle(Confirm), Some(MenuAction::Resume));
}

#[test]
fn save_grid_saves_empty_slot_and_confirms_overwrite() {
    let mut m = menu();
    assert_eq!(press(&mut m, &open_root(RootItem::SaveState)), None);
    assert_eq!(m.screen(), Screen::Slots(SlotMode::Save));
    assert_eq!(
        m.handle(Confirm),
        Some(MenuAction::SaveSlot(0)),
        "empty: no question"
    );
    // Slot 1 is used: ask first, "No" is the default.
    assert_eq!(press(&mut m, &[Right, Confirm]), None);
    assert_eq!(m.screen(), Screen::ConfirmOverwrite(1));
    assert!(!m.confirm_yes());
    assert_eq!(m.handle(Confirm), None, "No keeps the old state");
    assert_eq!(m.screen(), Screen::Slots(SlotMode::Save));
    assert_eq!(
        press(&mut m, &[Confirm, Left, Confirm]),
        Some(MenuAction::SaveSlot(1))
    );
    // Back from the question returns to the grid, from the grid to the root.
    assert_eq!(press(&mut m, &[Confirm, Back]), None);
    assert_eq!(m.screen(), Screen::Slots(SlotMode::Save));
    m.handle(Back);
    assert_eq!(m.screen(), Screen::Root);
    assert_eq!(
        m.root_cursor(),
        root_index(RootItem::SaveState),
        "cursor kept"
    );
}

#[test]
fn grid_moves_by_row_and_stays_inside() {
    let mut m = menu();
    press(&mut m, &open_root(RootItem::SaveState));
    press(&mut m, &[Down]);
    assert_eq!(m.slot_cursor(), 5);
    press(&mut m, &[Down, Right, Right]);
    assert_eq!(m.slot_cursor(), 7, "bottom edge holds");
    press(&mut m, &[Up, Up, Left, Left, Left]);
    assert_eq!(m.slot_cursor(), 0, "top-left edge holds");
}

#[test]
fn load_grid_skips_empty_slots() {
    let mut m = menu();
    press(&mut m, &open_root(RootItem::LoadState));
    assert_eq!(m.screen(), Screen::Slots(SlotMode::Load));
    assert_eq!(m.slot_cursor(), 1, "starts on the first used slot");
    assert_eq!(m.handle(Right), None);
    assert_eq!(m.slot_cursor(), 7, "2..=6 are empty");
    assert_eq!(m.handle(Right), None);
    assert_eq!(m.slot_cursor(), 7, "nothing used further right");
    assert_eq!(m.handle(Confirm), Some(MenuAction::LoadSlot(7)));
}

#[test]
fn load_is_disabled_without_states() {
    let options = OptionValues::from_config(&ResolvedConfig::default(), None, None);
    let mut m = PauseMenuState::new(vec![SlotSummary::default(); SLOT_COUNT], options, 3);
    assert!(!m.load_enabled());
    assert_eq!(press(&mut m, &open_root(RootItem::LoadState)), None);
    assert_eq!(m.screen(), Screen::Root);
    m.set_slot(
        3,
        SlotSummary {
            used: true,
            saved_at: None,
        },
    );
    press(&mut m, &[Confirm]);
    assert_eq!(m.screen(), Screen::Slots(SlotMode::Load));
    assert_eq!(m.slot_cursor(), 3);
}

#[test]
fn options_cycle_values_and_run_actions_on_confirm_only() {
    let mut m = menu();
    press(&mut m, &open_root(RootItem::Options));
    assert_eq!(m.screen(), Screen::Options);
    assert_eq!(
        m.handle(Right),
        Some(MenuAction::SetShader("crt-easymode".into()))
    );
    assert_eq!(m.handle(Left), Some(MenuAction::SetShader("none".into())));
    let last = config_layers::BUNDLED_PRESETS.last().unwrap();
    assert_eq!(
        m.handle(Left),
        Some(MenuAction::SetShader((*last).into())),
        "wraps"
    );
    assert_eq!(
        press(&mut m, &[Down, Left]),
        Some(MenuAction::SetScaling(VideoScaling::Stretch))
    );
    assert_eq!(
        press(&mut m, &[Down, Right]),
        Some(MenuAction::SetRunAhead(1))
    );
    assert_eq!(
        press(&mut m, &[Right, Right]),
        Some(MenuAction::SetRunAhead(0)),
        "0..=2 wraps"
    );
    assert_eq!(
        press(&mut m, &[Down, Confirm]),
        Some(MenuAction::SetRewind(true))
    );
    assert_eq!(
        press(&mut m, &[Down, Right]),
        None,
        "action rows ignore Left/Right"
    );
    assert_eq!(m.handle(Confirm), Some(MenuAction::SaveForGame));
    assert_eq!(
        press(&mut m, &[Down, Confirm]),
        Some(MenuAction::SaveAsDefault)
    );
    let settings_reset = Some(MenuAction::ResetGameSettings);
    assert_eq!(
        press(&mut m, &[Down, Confirm]),
        settings_reset,
        "not the core"
    );
    m.set_message("Saved for this game");
    assert_eq!(m.message(), Some("Saved for this game"));
    assert_eq!(m.handle(Back), None);
    assert_eq!(
        (m.screen(), m.message()),
        (Screen::Root, None),
        "input clears the message"
    );
}

#[test]
fn refused_run_ahead_and_rewind_rows_are_inert() {
    let options = OptionValues::from_config(&ResolvedConfig::default(), Some("ra"), Some("rw"));
    let mut m = PauseMenuState::new(vec![SlotSummary::default(); SLOT_COUNT], options, 0);
    press(&mut m, &[open_root(RootItem::Options), vec![Down]].concat()); // on Scaling
    for item in [OptionItem::RunAhead, OptionItem::Rewind] {
        press(&mut m, &[Down]);
        assert_eq!(OPTION_ITEMS[m.option_cursor()], item);
        for nav in [Right, Left, Confirm] {
            assert_eq!(m.handle(nav), None, "{item:?}");
        }
    }
    assert_eq!((m.options().run_ahead, m.options().rewind), (0, false));
    assert_eq!(m.options().rewind_unavailable, Some("rw"));
}

#[test]
fn custom_shader_is_an_extra_choice() {
    let mut config = ResolvedConfig::default();
    config.video.shader = "C:/presets/mine.slangp".into();
    let options = OptionValues::from_config(&config, None, None);
    // none + bundled presets + the custom path
    assert_eq!(
        options.shaders.len(),
        config_layers::BUNDLED_PRESETS.len() + 2
    );
    assert_eq!(options.shader_value(), "C:/presets/mine.slangp");
}
