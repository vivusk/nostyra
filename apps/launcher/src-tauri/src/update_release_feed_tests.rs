//! Feed parsing on a trimmed copy of the real v0.4.0 API answer (fetched 2026-11-09
//! with `gh api repos/vivusk/nostyra/releases/latest`: tag, name, html_url,
//! published_at, assets, first ~2 KB of the body), plus the offline client. No test
//! touches the network.

use http_download_util::DownloadError;

use super::*;

/// Trimmed real `GET /repos/vivusk/nostyra/releases/latest` answer for v0.4.0.
pub(crate) const V040_RELEASE_JSON: &str = r#"{
 "url": "https://api.github.com/repos/vivusk/nostyra/releases/438655116",
 "html_url": "https://github.com/vivusk/nostyra/releases/tag/v0.4.0",
 "tag_name": "v0.4.0",
 "name": "Nostyra 0.4.0 — PlayStation 2, Atari 2600 / 7800 / Lynx",
 "draft": false,
 "prerelease": false,
 "published_at": "2026-11-09T01:33:21Z",
 "assets": [
  {
   "name": "Nostyra-0.4.0-windows-x64-portable.zip",
   "size": 27802179,
   "digest": "sha256:dbfc88173e3f2674e9de793ec3b82dc27429e65b9caaa1b942d06e80d06fcfca"
  }
 ],
 "body": "> **Old Worlds. New Ways.**\n\n## Download\n\n| File | Size | SHA-256 |\n|---|---|---|\n| `Nostyra-0.4.0-windows-x64-portable.zip` | 26.5 MB | `dbfc88173e3f2674e9de793ec3b82dc27429e65b9caaa1b942d06e80d06fcfca` |\n\nOr play in the browser (desktop Chrome / Edge): **https://nostyra.com/play** (the 0.4.0 web edition with Atari\n2600 / 7800 / Lynx rolls out separately; PlayStation 2 is desktop only).\n\nUpgrading from 0.2.0 or any 0.3.x: extract the new zip over the old folder (or copy your `data\\` folder into the\nnew one); your library, saves and settings carry over. The first start upgrades the library database for the new\nsystems; older versions cannot open it afterwards, so keep a copy of `data\\` if you may want to go back.\n\nNostyra is a free, open-source (GPL-3.0-or-later) retro game launcher and libretro frontend. Point it at\nyour ROM folder, organised however you like, and it builds a library with covers and metadata. Games\nrun through libretro emulator cores, which Nostyra downloads the first time you play a system.\n\nThis is an early release. SNES is the only system tested end to end so far. The other systems pass\nautomated tests but are marked **Experimental** until they get manual testing.\n\n## What's new in 0.4.0\n\nFour new systems: **Sony PlayStation 2**, **Atari 2600**, **Atari 7800** and **Atari Lynx** (20 systems in all).\nThey pass automated tests and ran on real games during development, but are **Experimental** like the other\nsystems until they get manual testing. Rescan your ROM folders once (Settings → ROM folders) to pick up the new\nfile types.\n\n- **Sony PlayStation 2** (desktop only) on **PCEE2**, the PCSX2 libretro core (PCSX2 v2.9, OpenGL).\n  - **Needs a BIOS dumped from your own PS2** — there is no built-in replacement and Nostyra never includes or\n    downloads one. Press Play on a PS2 game and Nostyra asks for the file (**Choose BIOS file…**: `.BIN` or"
}"#;

#[test]
fn real_v040_answer_maps_to_cached_release() {
    let release = parse_release_json(V040_RELEASE_JSON.as_bytes()).unwrap();
    assert_eq!(release.version, "0.4.0");
    assert_eq!(
        release.title,
        "Nostyra 0.4.0 \u{2014} PlayStation 2, Atari 2600 / 7800 / Lynx"
    );
    assert_eq!(
        release.url,
        "https://github.com/vivusk/nostyra/releases/tag/v0.4.0"
    );
    assert_eq!(
        release.published_at.as_deref(),
        Some("2026-11-09T01:33:21Z")
    );
    assert!(release.has_portable_zip);
    assert!(release
        .excerpt
        .starts_with("Four new systems: Sony PlayStation 2"));
}

#[test]
fn optional_fields_may_be_missing() {
    let json = r#"{"tag_name":"v0.5.0","html_url":"https://github.com/vivusk/nostyra/releases/tag/v0.5.0"}"#;
    let release = parse_release_json(json.as_bytes()).unwrap();
    assert_eq!(release.version, "0.5.0");
    assert_eq!(release.title, "Nostyra 0.5.0");
    assert_eq!(release.published_at, None);
    assert_eq!(release.excerpt, "");
    assert!(!release.has_portable_zip);
}

#[test]
fn missing_tag_or_bad_json_is_bad_feed() {
    let no_tag = r#"{"html_url":"https://github.com/vivusk/nostyra/releases/tag/v0.5.0"}"#;
    for bytes in [no_tag.as_bytes(), b"<html>rate limited</html>"] {
        let err = parse_release_json(bytes).unwrap_err();
        assert!(matches!(err, FeedError::BadFeed(_)));
        assert_eq!(err.class(), "bad_feed");
    }
}

#[test]
fn non_semver_tag_is_bad_feed() {
    let json =
        r#"{"tag_name":"v0.5","html_url":"https://github.com/vivusk/nostyra/releases/tag/v0.5"}"#;
    assert!(matches!(
        parse_release_json(json.as_bytes()),
        Err(FeedError::BadFeed(_))
    ));
}

#[test]
fn offline_client_fails_transient_without_io() {
    let http = http_download_util::build_http_client("Nostyra-test", true);
    let err = fetch_latest_release(&http, Some("W/\"abc\"")).unwrap_err();
    assert!(matches!(err, FeedError::Http(DownloadError::Transient(_))));
    assert_eq!(err.class(), "transient");
}
