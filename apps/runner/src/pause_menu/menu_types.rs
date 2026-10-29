//! Vocabulary of the pause menu shared by its state machine, input adapter, view
//! and the loop glue: navigation events (and the keys / pad buttons that produce
//! them), key / pad routes, actions, screens and grid cells.

use config_layers::VideoScaling;
use sdl3::gamepad::Button;
use winit::keyboard::KeyCode;

use crate::runner_cli_args::MAX_STATE_SLOT;

/// Slots in the grid (0..=MAX_STATE_SLOT).
pub const SLOT_COUNT: usize = MAX_STATE_SLOT as usize + 1;
/// Grid columns (2 rows × 5).
pub const GRID_COLUMNS: usize = 5;

/// One navigation input (keyboard or pad, after key repeat).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavEvent {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
}

/// What the loop must do in response to a navigation event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuAction {
    Resume,
    SaveSlot(u8),
    LoadSlot(u8),
    SetShader(String),
    SetScaling(VideoScaling),
    SetRunAhead(u8),
    SetRewind(bool),
    SaveForGame,
    SaveAsDefault,
    /// Drop this game's menu option overrides (Options → "Reset game settings").
    ResetGameSettings,
    /// Soft-reset the core (root → "Reset game", after Yes).
    ResetCore,
    /// Insert disc N (0-based) of a multi-disc game (root → "Disc").
    SwapDisc(u32),
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootItem {
    Resume,
    SaveState,
    LoadState,
    /// Disc list of a multi-disc game (hidden for single-disc games and cartridges).
    Disc,
    /// RetroAchievements list of the game (only while an achievement session runs).
    Achievements,
    /// Core soft reset ("Reset game"); asks first.
    ResetCore,
    Options,
    Quit,
}

/// Every root row in display order; [`root_items`] drops the ones a game lacks.
const ALL_ROOT_ITEMS: [RootItem; 8] = [
    RootItem::Resume,
    RootItem::SaveState,
    RootItem::LoadState,
    RootItem::Disc,
    RootItem::Achievements,
    RootItem::ResetCore,
    RootItem::Options,
    RootItem::Quit,
];

/// Root rows: "Disc" only for a game with more than one disc, "Achievements" only
/// with an achievement session.
pub fn root_items(has_discs: bool, has_achievements: bool) -> Vec<RootItem> {
    ALL_ROOT_ITEMS
        .into_iter()
        .filter(|item| match item {
            RootItem::Disc => has_discs,
            RootItem::Achievements => has_achievements,
            _ => true,
        })
        .collect()
}

/// Discs of the running game (labels in disc order) and the inserted one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscList {
    pub labels: Vec<String>,
    pub current: usize,
}

impl DiscList {
    /// More than one disc: the menu offers swapping.
    pub fn is_multi_disc(&self) -> bool {
        self.labels.len() > 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotMode {
    Save,
    Load,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Root,
    Slots(SlotMode),
    /// Save grid asking before replacing the state in this slot.
    ConfirmOverwrite(u8),
    /// Root asking before Quit.
    ConfirmQuit,
    /// Root asking before the core reset ("Reset game").
    ConfirmReset,
    Options,
    /// Disc list (multi-disc games).
    Discs,
    /// RetroAchievements list (scrolls; Back returns to the root).
    Achievements,
}

/// One grid cell: whether the slot holds a state, and when it was saved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotSummary {
    pub used: bool,
    /// Local time, e.g. "2026-10-18 14:02" (missing when the sidecar is unreadable).
    pub saved_at: Option<String>,
}

/// Next / previous index in `0..len`, wrapping.
pub fn cycle(index: usize, len: usize, forward: bool) -> usize {
    match (len, forward) {
        (0, _) => 0,
        (_, true) => (index + 1) % len,
        (_, false) => (index + len - 1) % len,
    }
}

/// What a key event means for the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRoute {
    /// Not the menu's: game input / hotkeys as usual.
    Pass,
    /// Eaten (menu open, or a key still held from the menu).
    Consumed,
    Nav(NavEvent),
}

/// What a pad input event means for the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadRoute {
    Ignore,
    Open,
    Nav(NavEvent),
    /// Menu closed, a fresh press that is not the menu's: pad hotkeys may use it.
    Pass,
}

/// Arrows move, Enter confirms, Esc / Backspace go back (Back on the root resumes).
/// Keys bound to the `pause_menu` hotkey also go back (see `PauseMenuInput::on_key`).
pub fn key_nav(key: KeyCode) -> Option<NavEvent> {
    Some(match key {
        KeyCode::ArrowUp => NavEvent::Up,
        KeyCode::ArrowDown => NavEvent::Down,
        KeyCode::ArrowLeft => NavEvent::Left,
        KeyCode::ArrowRight => NavEvent::Right,
        KeyCode::Enter | KeyCode::NumpadEnter => NavEvent::Confirm,
        KeyCode::Escape | KeyCode::Backspace => NavEvent::Back,
        _ => return None,
    })
}

/// D-pad moves, South (A / Cross) confirms, East (B / Circle) and Guide go back.
pub fn pad_nav(button: Button) -> Option<NavEvent> {
    Some(match button {
        Button::DPadUp => NavEvent::Up,
        Button::DPadDown => NavEvent::Down,
        Button::DPadLeft => NavEvent::Left,
        Button::DPadRight => NavEvent::Right,
        Button::South => NavEvent::Confirm,
        Button::East | Button::Guide => NavEvent::Back,
        _ => return None,
    })
}
