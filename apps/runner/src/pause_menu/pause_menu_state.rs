//! Pause-menu state machine (pure, no I/O): which screen is shown, where the cursor
//! is, and which [`MenuAction`] a navigation event asks the loop to perform.
//!
//! ```text
//! Root: Resume · Save state → slot grid (overwrite confirm) · Load state → grid
//!       (used slots only) · Reset game (Yes / No confirm) · Options (shader,
//!       scaling, run-ahead, rewind, save / reset-settings actions) · Quit (Yes / No
//!       confirm) · Disc (multi-disc games only: list, current disc marked) ·
//!       Achievements (achievement session only: scrolling list)
//! ```
//! Back goes one level up; Back on the root resumes.

use super::achievement_list_page_state::AchievementListPageState;
use super::menu_option_values::OptionValues;
use super::menu_types::{
    cycle, DiscList, MenuAction, NavEvent, RootItem, Screen, SlotMode, SlotSummary, GRID_COLUMNS,
};

mod achievement_list_screen;
mod confirm_question;
mod disc_list_screen;
mod options_screen;

pub struct PauseMenuState {
    screen: Screen,
    root: usize,
    slot: usize,
    option: usize,
    /// Overwrite / reset / quit question: "Yes" highlighted (defaults to "No").
    confirm_yes: bool,
    slots: Vec<SlotSummary>,
    options: OptionValues,
    /// Result line (e.g. "Saved for this game"); cleared by the next input.
    message: Option<String>,
    discs: DiscList,
    disc: usize,
    achievements: AchievementListPageState,
}

impl PauseMenuState {
    /// Root screen; the grids start at `current_slot`.
    pub fn new(slots: Vec<SlotSummary>, options: OptionValues, current_slot: u8) -> Self {
        let slot = usize::from(current_slot).min(slots.len().saturating_sub(1));
        Self {
            screen: Screen::Root,
            root: 0,
            slot,
            option: 0,
            confirm_yes: false,
            slots,
            options,
            message: None,
            discs: DiscList::default(),
            disc: 0,
            achievements: AchievementListPageState::default(),
        }
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }
    pub fn root_cursor(&self) -> usize {
        self.root
    }
    pub fn slot_cursor(&self) -> usize {
        self.slot
    }
    pub fn option_cursor(&self) -> usize {
        self.option
    }
    pub fn confirm_yes(&self) -> bool {
        self.confirm_yes
    }
    pub fn slots(&self) -> &[SlotSummary] {
        &self.slots
    }
    pub fn options(&self) -> &OptionValues {
        &self.options
    }
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Load state is offered only when some slot holds a state.
    pub fn load_enabled(&self) -> bool {
        self.slots.iter().any(|slot| slot.used)
    }

    pub fn set_slot(&mut self, slot: u8, summary: SlotSummary) {
        if let Some(cell) = self.slots.get_mut(usize::from(slot)) {
            *cell = summary;
        }
    }

    pub fn set_options(&mut self, options: OptionValues) {
        self.options = options;
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
    }

    /// Applies one input; returns the action the loop must perform, if any.
    pub fn handle(&mut self, nav: NavEvent) -> Option<MenuAction> {
        self.message = None;
        match self.screen {
            Screen::Root => self.handle_root(nav),
            Screen::Slots(mode) => self.handle_slots(mode, nav),
            Screen::ConfirmOverwrite(_) | Screen::ConfirmQuit | Screen::ConfirmReset => {
                self.handle_confirm(nav)
            }
            Screen::Options => self.handle_options(nav),
            Screen::Discs => self.handle_discs(nav),
            Screen::Achievements => self.handle_achievements(nav),
        }
    }

    fn handle_root(&mut self, nav: NavEvent) -> Option<MenuAction> {
        match nav {
            NavEvent::Up | NavEvent::Down => {
                self.root = cycle(self.root, self.root_items().len(), nav == NavEvent::Down);
            }
            NavEvent::Back => return Some(MenuAction::Resume),
            NavEvent::Confirm => match self.root_items()[self.root] {
                RootItem::Resume => return Some(MenuAction::Resume),
                RootItem::Quit => self.ask(Screen::ConfirmQuit),
                RootItem::ResetCore => self.ask(Screen::ConfirmReset),
                RootItem::SaveState => self.screen = Screen::Slots(SlotMode::Save),
                RootItem::LoadState if self.load_enabled() => {
                    if !self.slots[self.slot].used {
                        self.slot = self.slots.iter().position(|s| s.used).unwrap_or(0);
                    }
                    self.screen = Screen::Slots(SlotMode::Load);
                }
                RootItem::LoadState => {} // disabled: no saved states
                RootItem::Options => self.screen = Screen::Options,
                RootItem::Disc => self.open_discs(),
                RootItem::Achievements => self.screen = Screen::Achievements,
            },
            NavEvent::Left | NavEvent::Right => {}
        }
        None
    }

    fn handle_slots(&mut self, mode: SlotMode, nav: NavEvent) -> Option<MenuAction> {
        let columns = GRID_COLUMNS as isize;
        let step = match nav {
            NavEvent::Left => -1,
            NavEvent::Right => 1,
            NavEvent::Up => -columns,
            NavEvent::Down => columns,
            NavEvent::Back => {
                self.screen = Screen::Root;
                return None;
            }
            NavEvent::Confirm => return self.confirm_slot(mode),
        };
        self.move_slot(step, mode == SlotMode::Load);
        None
    }

    fn confirm_slot(&mut self, mode: SlotMode) -> Option<MenuAction> {
        let slot = self.slot as u8;
        let used = self.slots.get(self.slot).is_some_and(|s| s.used);
        match mode {
            SlotMode::Save if used => {
                self.ask(Screen::ConfirmOverwrite(slot));
                None
            }
            SlotMode::Save => Some(MenuAction::SaveSlot(slot)),
            SlotMode::Load => used.then_some(MenuAction::LoadSlot(slot)),
        }
    }

    /// Moves by `step` cells; the load grid skips empty slots (stays put if none).
    fn move_slot(&mut self, step: isize, used_only: bool) {
        let mut next = self.slot as isize;
        loop {
            next += step;
            let Some(cell) = usize::try_from(next).ok().and_then(|i| self.slots.get(i)) else {
                return;
            };
            if cell.used || !used_only {
                self.slot = next as usize;
                return;
            }
        }
    }
}
