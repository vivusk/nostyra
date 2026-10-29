//! In-game pause menu (Esc, the bound `pause_menu` key / pad input (F10 by default),
//! pad Guide, or Select+Start held 0.5 s): resume, save / load with a thumbnail grid,
//! live options with "save for this game / as default", reset game and quit (both ask
//! Yes / No first), and the achievement list while a RetroAchievements session runs.
//! Drawn on the overlay layer after the shader. This module is the menu itself, free
//! of emulation: a pure state machine (`pause_menu_state`), the key / pad adapter
//! (`pause_menu_input`) and the painter (`pause_menu_view`); the loop side lives in
//! `emulation_main_loop::pause_glue`.

mod achievement_list_page;
mod achievement_list_page_state;
#[cfg(test)]
pub(crate) mod achievement_list_page_tests;
mod achievement_list_view;
mod menu_labels;
mod menu_option_values;
mod menu_types;
#[cfg(test)]
mod pause_menu_binding_input_tests;
#[cfg(test)]
mod pause_menu_disc_tests;
mod pause_menu_input;
#[cfg(test)]
mod pause_menu_input_tests;
#[cfg(test)]
mod pause_menu_root_tests;
mod pause_menu_state;
#[cfg(test)]
mod pause_menu_state_tests;
mod pause_menu_view;
#[cfg(test)]
mod pause_menu_view_tests;
mod slot_grid_view;
mod slot_thumbnail_cache;

use std::time::Instant;

use input_mapping::GamepadInput;
use video_renderer::{OverlayCanvas, TtfText};
use winit::keyboard::KeyCode;

pub use achievement_list_page::AchievementListPage;
pub use menu_option_values::OptionValues;
pub use menu_types::{DiscList, KeyRoute, MenuAction, NavEvent, PadRoute, SlotSummary, SLOT_COUNT};
pub use slot_thumbnail_cache::SlotThumbnailCache;

use crate::achievement_overlay::BadgeCache;
use crate::state_thumbnail_writer::Thumbnail;
use pause_menu_input::PauseMenuInput;
use pause_menu_state::PauseMenuState;

struct OpenMenu {
    state: PauseMenuState,
    thumbnails: SlotThumbnailCache,
}

#[derive(Default)]
pub struct PauseMenu {
    /// `Some` while the menu is shown (the game is paused).
    open: Option<OpenMenu>,
    input: PauseMenuInput,
    canvas: OverlayCanvas,
    text: TtfText, // achievement page text (fonts parsed on its first paint)
    /// Output size + badge revision painted for; `None` = repaint on the next present.
    painted_for: Option<((u32, u32), u64)>,
}

impl PauseMenu {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Shows the root screen (grids at `current_slot`; "Disc" for a multi-disc game).
    pub fn open(
        &mut self,
        slots: Vec<SlotSummary>,
        options: OptionValues,
        current_slot: u8,
        thumbnails: SlotThumbnailCache,
        discs: DiscList,
    ) {
        let mut state = PauseMenuState::new(slots, options, current_slot);
        state.set_discs(discs);
        self.open = Some(OpenMenu { state, thumbnails });
        self.input.on_open();
        self.painted_for = None;
    }

    /// Hides the menu; keys still held are swallowed until released.
    pub fn close(&mut self) {
        self.open = None;
        self.input.on_close();
    }

    /// Routes a key event; `menu_binding` = bound to the `pause_menu` hotkey.
    pub fn on_key(
        &mut self,
        key: KeyCode,
        pressed: bool,
        repeat: bool,
        menu_binding: bool,
        now: Instant,
    ) -> KeyRoute {
        let open = self.is_open();
        self.input
            .on_key(key, pressed, repeat, menu_binding, open, now)
    }

    /// Routes a pad input event; `menu_binding` = bound to the pad `pause_menu` hotkey.
    pub fn on_pad_input(
        &mut self,
        input: GamepadInput,
        pressed: bool,
        menu_binding: bool,
        now: Instant,
    ) -> PadRoute {
        let open = self.is_open();
        self.input
            .on_pad_input(input, pressed, menu_binding, open, now)
    }

    /// Whether the Select+Start chord (`held` on some pad) should open the menu now.
    pub fn poll_chord(&mut self, held: bool, now: Instant) -> bool {
        let open = self.is_open();
        self.input.poll_chord(held, open, now)
    }

    /// A repeated direction while one is held (menu open only).
    pub fn poll_repeat(&mut self, now: Instant) -> Option<NavEvent> {
        self.is_open()
            .then(|| self.input.poll_repeat(now))
            .flatten()
    }

    /// Window lost focus: forget held keys (no key-up will arrive).
    pub fn release_keys(&mut self) {
        self.input.release_keys();
    }

    pub fn handle(&mut self, nav: NavEvent) -> Option<MenuAction> {
        let open = self.open.as_mut()?;
        self.painted_for = None;
        open.state.handle(nav)
    }

    /// A slot was just saved from the menu: its cell and thumbnail change at once.
    pub fn set_slot(&mut self, slot: u8, summary: SlotSummary, thumbnail: Option<Thumbnail>) {
        if let Some(open) = self.open.as_mut() {
            open.state.set_slot(slot, summary);
            open.thumbnails.store(usize::from(slot), thumbnail);
            self.painted_for = None;
        }
    }

    pub fn set_options(&mut self, options: OptionValues) {
        if let Some(open) = self.open.as_mut() {
            open.state.set_options(options);
            self.painted_for = None;
        }
    }

    /// Achievement page content (`None` without a session: no root item).
    pub fn set_achievements(&mut self, page: Option<AchievementListPage>) {
        if let Some(open) = self.open.as_mut() {
            open.state.set_achievement_page(page);
            self.painted_for = None;
        }
    }

    /// Result line in the panel footer until the next input.
    pub fn set_message(&mut self, message: impl Into<String>) {
        if let Some(open) = self.open.as_mut() {
            open.state.set_message(message);
            self.painted_for = None;
        }
    }

    /// The menu overlay (`None` when closed), repainted after a change / landed badge.
    pub fn overlay(
        &mut self,
        output: (u32, u32),
        title: &str,
        badges: Option<&mut BadgeCache>,
    ) -> Option<&OverlayCanvas> {
        let open = self.open.as_mut()?;
        let painted = (output, badges.as_ref().map_or(0, |b| b.revision()));
        if self.painted_for != Some(painted) {
            pause_menu_view::paint_menu(
                &mut self.canvas,
                (output, title),
                &open.state,
                &mut open.thumbnails,
                (&mut self.text, badges),
            );
            self.painted_for = Some(painted);
        }
        Some(&self.canvas)
    }
}
