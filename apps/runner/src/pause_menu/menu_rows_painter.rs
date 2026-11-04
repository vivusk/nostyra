//! Text parts of the pause-menu panel: the header (screen name + game title), one
//! menu row (label left, value or "›" right, cursor row as a rounded pill with an
//! accent bar) and the footer (result message, hint, or a question with Yes / No
//! buttons). Noto Sans, so Unicode game titles draw as they are.

use video_renderer::{FontFace, OverlayCanvas, TtfText};

use super::menu_labels::{footer_hint, screen_title};
use super::menu_panel_painter::fill_rounded_rect;
use super::menu_types::Screen;
use super::pause_menu_state::PauseMenuState;
use super::pause_menu_view::{MenuMetrics, HIGHLIGHT_RGBA, MUTED_RGBA, TEXT_RGBA};
use crate::achievement_overlay::ACCENT_RGBA;

/// Values of enabled, unselected rows: a step quieter than labels.
const VALUE_RGBA: [u8; 4] = [176, 186, 210, 255];
/// Unchosen Yes / No button.
const BUTTON_RGBA: [u8; 4] = [38, 42, 56, 255];

/// Painter for one panel column (`left`, `inner` wide).
pub(super) struct Rows<'a> {
    pub canvas: &'a mut OverlayCanvas,
    pub text: &'a mut TtfText,
    pub left: u32,
    pub inner: u32,
    pub m: &'a MenuMetrics,
}

impl Rows<'_> {
    /// Screen name (small accent capitals) over the game title; returns the y below.
    pub(super) fn header(&mut self, y: u32, title: &str, screen: Screen) -> u32 {
        let label = screen_title(screen).to_uppercase();
        let small = (FontFace::Bold, self.m.small);
        self.draw_fit((self.left, y), small, &label, ACCENT_RGBA, self.inner);
        let title_y = y + self.m.small_line;
        let face = (FontFace::Bold, self.m.title_px);
        self.draw_fit((self.left, title_y), face, title, TEXT_RGBA, self.inner);
        title_y + self.m.title_line
    }

    /// Label left, optional `value` right-aligned; cursor row highlighted.
    pub(super) fn row(
        &mut self,
        y: u32,
        label: &str,
        value: Option<String>,
        selected: bool,
        enabled: bool,
    ) {
        let m = self.m;
        if selected {
            let pill = [self.left - m.pad / 2, y, self.inner + m.pad, m.row_h];
            fill_rounded_rect(self.canvas, pill, m.radius / 2 + 1, HIGHLIGHT_RGBA);
            let bar_w = (2 * m.stroke).max(3);
            let bar = [
                self.left - m.pad / 2 + m.stroke * 2,
                y + m.row_h / 4,
                bar_w,
                m.row_h / 2,
            ];
            fill_rounded_rect(self.canvas, bar, bar_w / 2, ACCENT_RGBA);
        }
        let text_y = y + (m.row_h - m.line) / 2;
        let (label_rgba, value_rgba) = match (enabled, selected) {
            (false, _) => (MUTED_RGBA, MUTED_RGBA),
            (true, true) => (TEXT_RGBA, TEXT_RGBA),
            (true, false) => (TEXT_RGBA, VALUE_RGBA),
        };
        let label_face = if selected {
            FontFace::Bold
        } else {
            FontFace::Regular
        };
        let mut label_w = self.inner;
        if let Some(value) = value {
            let value = self
                .text
                .ellipsize(FontFace::Regular, m.px, &value, self.inner / 2);
            let value_w = self.text.measure(FontFace::Regular, m.px, &value);
            let value_x = (self.left + self.inner - value_w) as i32;
            let face = (FontFace::Regular, m.px);
            self.text.draw(
                self.canvas,
                (value_x, text_y as i32),
                face,
                &value,
                value_rgba,
            );
            label_w = self.inner.saturating_sub(value_w + m.px);
        }
        let face = (label_face, m.px);
        self.draw_fit((self.left, text_y), face, label, label_rgba, label_w);
    }

    /// Result message, disabled-row reason, overwrite / reset / quit question or a key hint.
    pub(super) fn footer(&mut self, y: u32, state: &PauseMenuState) {
        let m = self.m;
        let question = match state.screen() {
            Screen::ConfirmOverwrite(slot) => Some(format!("Overwrite slot {slot}?")),
            Screen::ConfirmQuit => Some("Quit game?".to_string()),
            Screen::ConfirmReset => Some("Reset game?".to_string()),
            _ => None,
        };
        let text_y = y + (m.footer_h() - m.small_line) / 2;
        let Some(question) = question else {
            let rgba = if state.message().is_some() {
                TEXT_RGBA
            } else {
                MUTED_RGBA
            };
            let face = (FontFace::Regular, m.small);
            return self.draw_fit(
                (self.left, text_y),
                face,
                footer_hint(state),
                rgba,
                self.inner,
            );
        };
        // Yes / No buttons right-aligned, the question in the room left of them.
        let button_w =
            |text: &mut TtfText, label| text.measure(FontFace::Bold, m.small, label) + m.px;
        let buttons = [("Yes", state.confirm_yes()), ("No", !state.confirm_yes())];
        let total: u32 = buttons
            .iter()
            .map(|(label, _)| button_w(self.text, label))
            .sum();
        let mut x = (self.left + self.inner).saturating_sub(total + m.pad / 2);
        let face = (FontFace::Bold, m.small);
        let room = x.saturating_sub(self.left + m.pad / 2);
        self.draw_fit((self.left, text_y), face, &question, TEXT_RGBA, room);
        for (label, chosen) in buttons {
            let w = button_w(self.text, label);
            let rgba = if chosen { HIGHLIGHT_RGBA } else { BUTTON_RGBA };
            fill_rounded_rect(self.canvas, [x, y, w, m.footer_h()], m.radius / 2 + 1, rgba);
            let label_x = x + m.px / 2;
            let label_rgba = if chosen { TEXT_RGBA } else { MUTED_RGBA };
            self.text.draw(
                self.canvas,
                (label_x as i32, text_y as i32),
                face,
                label,
                label_rgba,
            );
            x += w + m.pad / 2;
        }
    }

    /// `text` at `(x, y)` in `face`, ellipsized to `max_width`.
    fn draw_fit(
        &mut self,
        (x, y): (u32, u32),
        face: (FontFace, u32),
        text: &str,
        rgba: [u8; 4],
        max_width: u32,
    ) {
        let fitted = self.text.ellipsize(face.0, face.1, text, max_width);
        self.text
            .draw(self.canvas, (x as i32, y as i32), face, &fitted, rgba);
    }
}
