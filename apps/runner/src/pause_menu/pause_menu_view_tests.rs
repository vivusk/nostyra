//! View smoke tests: every screen paints at common and tiny sizes without panics.

use config_layers::ResolvedConfig;
use video_renderer::{OverlayCanvas, TtfText};

use super::menu_labels::footer_hint;
use super::menu_option_values::OptionValues;
use super::menu_types::{NavEvent, Screen, SlotSummary, SLOT_COUNT};
use super::pause_menu_state::PauseMenuState;
use super::pause_menu_view::{fit_text, paint_menu};
use super::slot_thumbnail_cache::SlotThumbnailCache;
use crate::state_thumbnail_writer::Thumbnail;
use NavEvent::{Back, Confirm, Down, Right, Up};

/// Navigation from the root to each screen.
const SCREENS: &[&[NavEvent]] = &[
    &[],
    &[Down, Confirm],
    &[Down, Confirm, Right, Confirm], // overwrite question on slot 1
    &[Down, Down, Confirm],
    &[Up, Up, Confirm, Down, Down],
    &[Up, Confirm, Right],        // quit question, "Yes" highlighted
    &[Down, Down, Down, Confirm], // "Reset game?" question, "No" highlighted
];

/// `unavailable`: reason for both run-ahead and rewind rows (`None`: both usable).
fn menu(unavailable: Option<&'static str>) -> PauseMenuState {
    let slots = (0..SLOT_COUNT)
        .map(|n| SlotSummary {
            used: n % 2 == 1,
            saved_at: (n % 2 == 1).then(|| "2026-10-18 14:02".to_string()),
        })
        .collect();
    let options = OptionValues::from_config(&ResolvedConfig::default(), unavailable, unavailable);
    PauseMenuState::new(slots, options, 0)
}

fn cache_with_slot_1() -> SlotThumbnailCache {
    let mut cache =
        SlotThumbnailCache::new(vec![std::path::PathBuf::from("missing.png"); SLOT_COUNT]);
    let rgba = vec![200; 256 * 224 * 4];
    cache.store(
        1,
        Some(Thumbnail {
            width: 256,
            height: 224,
            rgba,
        }),
    );
    cache
}

#[test]
fn every_screen_paints_at_every_size() {
    for size in [
        (896, 672),
        (1920, 1080),
        (3840, 2160),
        (300, 200),
        (8, 8),
        (1, 1),
    ] {
        for navs in SCREENS {
            let mut state = menu(Some("Core states incomplete"));
            navs.iter().for_each(|nav| {
                state.handle(*nav);
            });
            let mut canvas = OverlayCanvas::default();
            let mut cache = cache_with_slot_1();
            paint_menu(
                &mut canvas,
                (size, "Chrono Trigger (USA) — ü"),
                &state,
                &mut cache,
                (&mut TtfText::default(), None),
            );
            assert_eq!(canvas.size(), size);
            assert!(
                // ≥ 70 % opaque everywhere: RA hardcore's pause-overlay minimum.
                canvas.pixels().chunks(4).all(|px| px[3] >= 179),
                "fully dimmed at {size:?}"
            );
        }
    }
}

#[test]
fn panel_and_thumbnail_are_drawn_on_the_save_grid() {
    let mut state = menu(None);
    state.handle(Down);
    state.handle(Confirm);
    assert!(matches!(state.screen(), Screen::Slots(_)));
    let mut canvas = OverlayCanvas::default();
    paint_menu(
        &mut canvas,
        ((896, 672), "Game"),
        &state,
        &mut cache_with_slot_1(),
        (&mut TtfText::default(), None),
    );
    let thumb_px = canvas
        .pixels()
        .chunks(4)
        .filter(|px| *px == [200, 200, 200, 200])
        .count();
    assert!(thumb_px > 1000, "slot 1 thumbnail blitted ({thumb_px} px)");
    state.handle(Back);
    paint_menu(
        &mut canvas,
        ((896, 672), "Game"),
        &state,
        &mut cache_with_slot_1(),
        (&mut TtfText::default(), None),
    );
    assert_eq!(
        canvas
            .pixels()
            .chunks(4)
            .filter(|px| *px == [200, 200, 200, 200])
            .count(),
        0
    );
}

#[test]
fn reset_question_is_one_of_the_painted_screens() {
    let mut state = menu(None);
    SCREENS[6].iter().for_each(|nav| {
        state.handle(*nav);
    });
    assert_eq!(state.screen(), Screen::ConfirmReset);
}

#[test]
fn footer_shows_the_reason_of_the_disabled_row_under_the_cursor() {
    let hint = "Enter / A: select   Esc / B: back";
    let mut state = menu(Some("Core save states are not frame-exact"));
    for nav in [Up, Up, Confirm] {
        state.handle(nav); // root → Options (cursor on Shader)
    }
    assert_eq!(state.screen(), Screen::Options);
    assert_eq!(footer_hint(&state), hint);
    for expected in [
        hint,
        "Core save states are not frame-exact",
        "Core save states are not frame-exact",
        hint,
    ] {
        state.handle(Down); // Scaling, Run-ahead, Rewind, Save for this game
        assert_eq!(footer_hint(&state), expected);
    }
    let mut usable = menu(None);
    for nav in [Up, Up, Confirm, Down, Down, Down] {
        usable.handle(nav); // cursor on Rewind
    }
    assert_eq!(footer_hint(&usable), hint);
}

#[test]
fn long_text_is_cut_to_fit() {
    assert_eq!(fit_text("Resume", 100, 1), "Resume");
    let cut = fit_text("A very long ROM name that does not fit", 60, 2);
    assert!(cut.ends_with("..") && cut.chars().count() == 5, "{cut}");
    assert_eq!(fit_text("abc", 0, 2), "", "no room at all");
}
