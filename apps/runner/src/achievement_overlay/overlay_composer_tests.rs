//! Composer: nothing to show → no overlay; status, popup and indicator regions;
//! redraw only on change; leaderboard setting; every window size draws.

use std::time::{Duration, Instant};

use super::*;

pub(super) const OUTPUT: (u32, u32) = (1280, 720);
/// 4:3 picture centred in the 16:9 window.
pub(super) const GAME: [u32; 4] = [160, 0, 960, 720];

fn alpha_in(canvas: &video_renderer::OverlayCanvas, rect: [u32; 4]) -> usize {
    let (w, _) = canvas.size();
    let [x0, y0, rw, rh] = rect;
    (y0..y0 + rh)
        .flat_map(|y| (x0..x0 + rw).map(move |x| (x, y)))
        .filter(|(x, y)| canvas.pixels()[((y * w + x) * 4 + 3) as usize] > 0)
        .count()
}

pub(super) fn badge(name: &str) -> BadgeKey {
    BadgeKey::Achievement {
        name: name.into(),
        locked: false,
    }
}

pub(super) fn unlock_popup() -> Popup {
    Popup {
        kind: PopupKind::Unlock,
        icon: Some(badge("00101")),
        header: "Achievement unlocked · 10 points".into(),
        title: "Tiếng Việt: Pokémon Привет".into(),
        detail: "Collect every coin in the first world without losing a single life".into(),
    }
}

#[test]
fn nothing_visible_means_no_overlay_and_no_redraw_while_unchanged() {
    let data = tempfile::tempdir().unwrap();
    let mut overlay = AchievementOverlay::new(data.path(), true, true);
    let mut composer = OverlayComposer::default();
    assert!(composer.compose(OUTPUT, GAME, None, &mut overlay).is_none());
    let canvas = composer
        .compose(OUTPUT, GAME, Some("SLOT 1 · SAVED"), &mut overlay)
        .unwrap();
    assert!(
        alpha_in(canvas, [160, 620, 300, 100]) > 0,
        "status bottom-left of the picture"
    );
    assert_eq!(alpha_in(canvas, [400, 0, 480, 200]), 0, "no popup");
    let now = Instant::now();
    overlay.announce(unlock_popup());
    assert!(overlay.tick(now), "an unlock rings");
    let version = composer
        .compose(OUTPUT, GAME, None, &mut overlay)
        .unwrap()
        .version();
    let again = composer
        .compose(OUTPUT, GAME, None, &mut overlay)
        .unwrap()
        .version();
    assert_eq!(version, again, "unchanged: no repaint, no upload");
    let canvas = composer.compose(OUTPUT, GAME, None, &mut overlay).unwrap();
    assert!(
        alpha_in(canvas, [400, 0, 480, 150]) > 1000,
        "popup at the top centre"
    );
    assert_eq!(alpha_in(canvas, [160, 620, 300, 100]), 0, "status gone");
    overlay.indicators.set_tracker(1, "0:12.50");
    let canvas = composer.compose(OUTPUT, GAME, None, &mut overlay).unwrap();
    assert!(canvas.version() > again, "tracker update repaints");
    assert!(
        alpha_in(canvas, [960, 620, 160, 100]) > 0,
        "tracker bottom-right"
    );
    overlay.tick(now + Duration::from_secs(4));
    overlay.indicators.hide_tracker(1);
    assert!(composer.compose(OUTPUT, GAME, None, &mut overlay).is_none());
}

#[test]
fn leaderboard_popups_follow_the_setting_and_every_size_draws() {
    let data = tempfile::tempdir().unwrap();
    let mut overlay = AchievementOverlay::new(data.path(), false, false);
    overlay.announce(Popup {
        kind: PopupKind::Leaderboard,
        ..Popup::notice("Leaderboard submitted")
    });
    assert!(
        overlay.take_log().is_empty(),
        "hidden with leaderboard_popups = false"
    );
    overlay.announce(unlock_popup());
    assert!(
        overlay.tick(Instant::now()),
        "rings (the sound rule applies later)"
    );
    assert!(!should_chime(true, overlay.sound(), false), "sound off");
    overlay.indicators.show_challenge(101, badge("00101"));
    overlay
        .indicators
        .show_progress(badge("00102"), "5/10", Instant::now());
    for size in [(1, 1), (64, 48), (320, 240), (1920, 1080), (3840, 2160)] {
        let game = [0, 0, size.0, size.1];
        let mut composer = OverlayComposer::default();
        let canvas = composer
            .compose(size, game, Some("SAVED"), &mut overlay)
            .unwrap();
        assert_eq!(canvas.size(), size);
    }
}
