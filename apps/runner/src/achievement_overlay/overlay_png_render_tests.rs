//! Manual look at the overlay: with `NOSTYRA_OVERLAY_PNG_DIR` set, writes the placard
//! frame (status line + trackers + challenge and progress indicators), the unlock
//! popup frame and the pause-menu achievement page as PNGs (composited over a grey
//! checkerboard so translucency shows). Without the variable the test does nothing.

use std::path::Path;
use std::time::{Duration, Instant};

use super::badge_cache::tests::png;
use super::overlay_composer_tests::{badge, unlock_popup, GAME, OUTPUT};
use super::*;

/// Badge files in the shared cache layout (game icon, two badges + locked variants).
fn seed_badges(data: &Path) {
    let dir = achievements::badge_cache_dir(data);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("game_085573.png"), png(64, [70, 130, 220, 255])).unwrap();
    for (name, rgba) in [
        ("00101", [240, 180, 40, 255]),
        ("00102", [60, 190, 110, 255]),
    ] {
        std::fs::write(dir.join(format!("{name}.png")), png(64, rgba)).unwrap();
        std::fs::write(
            dir.join(format!("{name}_lock.png")),
            png(64, [90, 90, 90, 255]),
        )
        .unwrap();
    }
}

#[test]
fn render_overlay_frames_to_png_when_asked() {
    let Some(dir) = std::env::var_os("NOSTYRA_OVERLAY_PNG_DIR") else {
        return; // manual look only
    };
    let dir = Path::new(&dir);
    let data = tempfile::tempdir().unwrap();
    seed_badges(data.path());
    let mut overlay = AchievementOverlay::new(data.path(), true, true);
    let game = achievements::GameInfo {
        title: "Nostyra Test Game — Tiếng Việt".into(),
        badge_name: "085573".into(),
        ..Default::default()
    };
    let summary = achievements::GameSummary {
        num_core_achievements: 12,
        num_unlocked_achievements: 3,
        points_core: 120,
        ..Default::default()
    };
    overlay.announce(game_placard::loaded(&game, &summary, false));
    overlay.announce(unlock_popup());
    overlay.indicators.set_tracker(7, "0:12.50");
    overlay.indicators.set_tracker(8, "Score 004200");
    overlay.indicators.show_challenge(101, badge("00101"));
    overlay.indicators.show_challenge(102, badge("00102"));
    overlay
        .indicators
        .show_progress(badge("00102"), "5/10", Instant::now());
    let now = Instant::now();
    let mut composer = OverlayComposer::default();
    for (name, at) in [
        ("placard", now),
        ("unlock-popup", now + Duration::from_secs(5)),
    ] {
        overlay.tick(at);
        let canvas = composer
            .compose(OUTPUT, GAME, Some("SLOT 1 · SAVED"), &mut overlay)
            .unwrap();
        save(canvas, &dir.join(format!("overlay-{name}.png")));
    }
    let mut menu = achievement_page_menu();
    let canvas = menu
        .overlay(OUTPUT, "game", Some(&mut overlay.badges))
        .unwrap();
    save(canvas, &dir.join("overlay-achievement-list.png"));
}

/// A pause menu showing the achievement page (opened through the root item).
fn achievement_page_menu() -> crate::pause_menu::PauseMenu {
    use crate::pause_menu::{DiscList, NavEvent, OptionValues, PauseMenu, SlotSummary};
    let mut menu = PauseMenu::default();
    let slots = vec![SlotSummary::default(); crate::pause_menu::SLOT_COUNT];
    let options = OptionValues::from_config(&config_layers::ResolvedConfig::default(), None, None);
    let thumbs = crate::pause_menu::SlotThumbnailCache::new(vec![Default::default(); slots.len()]);
    menu.open(slots, options, 0, thumbs, DiscList::default());
    let page = crate::pause_menu::achievement_list_page_tests::sample_page(14);
    menu.set_achievements(Some(page));
    for nav in [
        NavEvent::Down,
        NavEvent::Down,
        NavEvent::Down,
        NavEvent::Confirm,
        NavEvent::Down,
    ] {
        menu.handle(nav); // Resume → Save → Load → Achievements (no disc), then one row down
    }
    menu
}

/// Writes the canvas composited over a mid-grey "game" so translucency shows.
fn save(canvas: &video_renderer::OverlayCanvas, path: &Path) {
    let (w, h) = canvas.size();
    let mut out = image::RgbImage::new(w, h);
    for (i, px) in canvas.pixels().chunks(4).enumerate() {
        let a = u32::from(px[3]);
        let bg = if (i as u32 % w / 40 + i as u32 / w / 40).is_multiple_of(2) {
            96
        } else {
            128
        };
        let mix = |c: u8| ((u32::from(c) * a + bg * (255 - a)) / 255) as u8;
        out.put_pixel(
            i as u32 % w,
            i as u32 / w,
            image::Rgb([mix(px[0]), mix(px[1]), mix(px[2])]),
        );
    }
    out.save(path).unwrap();
}
