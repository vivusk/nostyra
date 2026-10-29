//! Session → overlay over the fake server (inline dispatch): placard then unlock
//! popup, badges fetched only for a window, the menu page, and leaderboards with
//! trackers (hardcore forced: rcheevos v12.5.0 tracks leaderboards only there, D3).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::time::{Duration, Instant};

use achievements::{fake_game_set_json, FakeTransport, RequestDispatch};
use config_layers::AchievementsConfig;
use video_renderer::ViewportRect;

use super::achievement_session_test_support::*;
use crate::on_screen_status::OnScreenStatus;
use crate::pause_menu::AchievementListPage;

const OUTPUT: (u32, u32) = (640, 480);
const GAME: ViewportRect = ViewportRect {
    x: 0.0,
    y: 0.0,
    width: 640.0,
    height: 480.0,
};
const LBOARD: u32 = 201;
const MEDIA: &str = "https://media.retroachievements.org";

fn rows_unlocked(page: &AchievementListPage) -> Vec<(String, bool)> {
    use crate::pause_menu::achievement_list_page_tests::row_states;
    row_states(page)
}

#[test]
fn window_session_fetches_badges_and_shows_placard_then_unlock() {
    let fake = fake_with_set();
    fake.serve_download(&format!("{MEDIA}/Badge/00101.png"), png_bytes());
    let mut h = harness(fake.clone(), RequestDispatch::Inline);
    h.session.attach_window(None); // window without audio: no chime stream
    h.frame();
    let asked = fake.downloads_requested();
    for file in [
        "Badge/00101.png",
        "Badge/00101_lock.png",
        "Badge/00102_lock.png",
        "Images/1234.png",
    ] {
        assert!(
            asked.contains(&format!("{MEDIA}/{file}")),
            "{file} in {asked:?}"
        );
    }
    assert!(h.session.take_list_changed(), "set loaded");
    let page = h.session.achievement_list_page().expect("page");
    assert_eq!(page.header.title, "Test Game");
    assert!(rows_unlocked(&page).iter().all(|(_, unlocked)| !unlocked));
    h.ram[0x10].set(5);
    h.frames(2);
    assert!(h.session.take_list_changed(), "unlock refreshes the page");
    let page = h.session.achievement_list_page().unwrap();
    assert!(rows_unlocked(&page).contains(&("Ach 101".into(), true)));
    let log = h.session.take_notices();
    assert_eq!(
        log[0], "RetroAchievements · Softcore mode: Test Game",
        "{log:?}"
    );
    assert!(
        log.contains(&"Achievement unlocked · 5 points: Ach 101".into()),
        "{log:?}"
    );
    let mut status = OnScreenStatus::default();
    let now = Instant::now();
    let canvas = h.session.overlay((OUTPUT, GAME), now, &mut status, false);
    assert!(canvas.is_some(), "placard on screen");
    let later = now + Duration::from_secs(5);
    assert!(h
        .session
        .overlay((OUTPUT, GAME), later, &mut status, false)
        .is_some());
    let done = later + Duration::from_secs(4);
    assert!(h
        .session
        .overlay((OUTPUT, GAME), done, &mut status, true)
        .is_none());
    assert!(
        h.session.badges_mut().get(&badge_101()).is_some(),
        "downloaded badge decodes"
    );
    h.session.shutdown();
}

#[test]
fn headless_sessions_never_fetch_badges() {
    let fake = fake_with_set();
    let mut h = harness(fake.clone(), RequestDispatch::Inline);
    h.frame();
    assert!(h.loaded());
    assert!(fake.downloads_requested().is_empty());
    h.session.shutdown();
}

#[test]
fn hardcore_leaderboard_popups_and_tracker() {
    for popups in [true, false] {
        let fake = std::sync::Arc::new(FakeTransport::new());
        let lb = "STA:0xH0020=1::CAN:0xH0021=1::SUB:0xH0020=2::VAL:0xH0022";
        let set = fake_game_set_json(
            1234,
            "Test Game",
            &[(BYTE_ACH, "0xH0010=5")],
            &[(LBOARD, lb)],
        );
        fake.respond("achievementsets", 200, &set);
        let config = AchievementsConfig {
            hardcore: true,
            leaderboard_popups: popups,
            ..AchievementsConfig::default()
        };
        let launch = HardcoreLaunch {
            hardcore_ui: true,
            ..HardcoreLaunch::default()
        };
        let mut h = harness_with(fake, RequestDispatch::Inline, &config, launch);
        h.session.attach_window(None);
        h.frame();
        h.ram[0x22].set(42);
        h.ram[0x20].set(1);
        h.frames(2);
        assert!(
            !h.session.router.overlay.indicators.is_empty(),
            "tracker shown"
        );
        h.ram[0x20].set(2);
        h.frames(2);
        assert!(
            h.session.router.overlay.indicators.is_empty(),
            "tracker hidden on submit"
        );
        let log = h.session.take_notices();
        let started = log
            .iter()
            .any(|l| l.starts_with("Leaderboard attempt started: LB 201"));
        let submitted = log
            .iter()
            .any(|l| l.starts_with("Leaderboard submitted: LB 201"));
        assert_eq!((started, submitted), (popups, popups), "{log:?}");
        h.session.shutdown();
    }
}

fn badge_101() -> crate::achievement_overlay::BadgeKey {
    crate::achievement_overlay::BadgeKey::Achievement {
        name: "00101".into(),
        locked: false,
    }
}

fn png_bytes() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(64, 64, image::Rgba([10, 200, 30, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}
