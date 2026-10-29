//! Badge cache: download through the transport (fake serves a PNG), shared cache
//! path, once per session, non-images refused, placeholder, resize.

use std::sync::Arc;

use achievements::{AchievementClient, FakeTransport, RequestDispatch};

use super::*;

const URL: &str = "https://media.retroachievements.org/Badge/00234.png";

pub(crate) fn png(size: u32, rgba: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(size, size, image::Rgba(rgba));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

fn client(fake: &Arc<FakeTransport>) -> AchievementClient {
    AchievementClient::new(fake.clone(), RequestDispatch::Inline, Box::new(|_, _| 0)).unwrap()
}

fn badge(name: &str) -> BadgeKey {
    BadgeKey::Achievement {
        name: name.into(),
        locked: false,
    }
}

#[test]
fn missing_badge_is_downloaded_once_into_the_shared_cache() {
    let data = tempfile::tempdir().unwrap();
    let fake = Arc::new(FakeTransport::new());
    fake.serve_download(URL, png(64, [200, 10, 20, 255]));
    let client = client(&fake);
    let mut cache = BadgeCache::new(data.path());
    assert!(cache.get(&badge("00234")).is_none(), "not there yet");
    cache.request(badge("00234"), &client);
    cache.request(badge("00234"), &client);
    assert_eq!(fake.downloads_requested(), [URL], "one GET per session");
    let file = achievements::badge_path(data.path(), "00234", false).unwrap();
    assert!(file.starts_with(data.path().join("cache").join("achievement-badges")));
    assert!(file.exists());
    let revision = cache.revision();
    assert!(cache.poll(), "landed");
    assert!(cache.revision() > revision);
    let image = cache.get(&badge("00234")).expect("decoded after landing");
    assert_eq!(&image.rgba[..4], &[200, 10, 20, 255]);
    assert_eq!(image.rgba.len(), (BADGE_SIZE * BADGE_SIZE * 4) as usize);
    let mut fresh = BadgeCache::new(data.path());
    fresh.request(badge("00234"), &client);
    assert_eq!(fake.downloads_requested().len(), 1, "file on disk: no GET");
}

#[test]
fn non_images_and_bad_names_never_land_and_draw_a_placeholder() {
    let data = tempfile::tempdir().unwrap();
    let fake = Arc::new(FakeTransport::new());
    fake.serve_download(URL, b"<html>proxy error</html>".to_vec());
    let client = client(&fake);
    let mut cache = BadgeCache::new(data.path());
    cache.request(badge("00234"), &client);
    cache.request(badge("../evil"), &client);
    assert_eq!(
        fake.downloads_requested(),
        [URL],
        "invalid name never asked"
    );
    assert!(!cache.poll());
    assert!(!achievements::badge_path(data.path(), "00234", false)
        .unwrap()
        .exists());
    let mut canvas = OverlayCanvas::default();
    canvas.reset(8, 8);
    cache.draw(&mut canvas, Some(&badge("00234")), [0, 0, 4, 4]);
    assert_eq!(&canvas.pixels()[..4], &PLACEHOLDER_RGBA);
}

#[test]
fn other_sizes_are_resized_and_icons_use_their_own_file() {
    let data = tempfile::tempdir().unwrap();
    let icon = BadgeKey::GameIcon("085573".into());
    let path = achievements::game_icon_path(data.path(), "085573").unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, png(32, [1, 2, 3, 255])).unwrap();
    let mut cache = BadgeCache::new(data.path());
    let image = cache.get(&icon).expect("icon decoded");
    assert_eq!(image.rgba.len(), (BADGE_SIZE * BADGE_SIZE * 4) as usize);
    let mut canvas = OverlayCanvas::default();
    canvas.reset(8, 8);
    cache.draw(&mut canvas, Some(&icon), [0, 0, 8, 8]);
    assert_eq!(&canvas.pixels()[..4], &[1, 2, 3, 255]);
}
