//! Version, asset, excerpt and URL rules of the update checker.

use super::*;
use crate::update_release_feed::tests::V040_RELEASE_JSON;

fn v(text: &str) -> Version {
    Version::parse(text).unwrap()
}

#[test]
fn tags_parse_with_one_optional_v() {
    assert_eq!(parse_release_version("v0.4.0"), Some(v("0.4.0")));
    assert_eq!(parse_release_version("V0.4.0"), Some(v("0.4.0")));
    assert_eq!(parse_release_version("0.4.0"), Some(v("0.4.0")));
    assert_eq!(parse_release_version("v0.5.0-rc.1"), Some(v("0.5.0-rc.1")));
    for bad in ["0.5", "v0.5", "latest", "", "vv0.4.0"] {
        assert_eq!(parse_release_version(bad), None, "{bad:?}");
    }
}

#[test]
fn newer_follows_semver_precedence() {
    let tag = |t: &str| parse_release_version(t).unwrap();
    assert!(!is_newer(&tag("v0.4.0"), &v("0.4.0")));
    assert!(is_newer(&tag("v0.4.1"), &v("0.4.0")));
    assert!(!is_newer(&tag("v0.3.2"), &v("0.4.0")));
    // Local pre-release is older than its final release.
    assert!(is_newer(&tag("v0.5.0"), &v("0.5.0-rc.1")));
    // Rule only: the endpoint never returns pre-releases.
    assert!(is_newer(&tag("v0.5.0-rc.1"), &v("0.4.0")));
    // Build metadata does not make a version newer.
    assert!(!is_newer(&tag("v0.4.0+build.7"), &v("0.4.0")));
}

#[test]
fn portable_zip_must_match_the_version() {
    let version = v("0.4.0");
    assert_eq!(
        portable_zip_name(&version),
        "Nostyra-0.4.0-windows-x64-portable.zip"
    );
    assert!(has_portable_zip(
        ["notes.txt", "Nostyra-0.4.0-windows-x64-portable.zip"],
        &version
    ));
    assert!(!has_portable_zip([], &version));
    assert!(!has_portable_zip(
        ["Nostyra-0.3.0-windows-x64-portable.zip"],
        &version
    ));
}

#[test]
fn excerpt_of_the_real_v040_notes_is_the_whats_new_paragraph() {
    let json: serde_json::Value = serde_json::from_str(V040_RELEASE_JSON).unwrap();
    let excerpt = notes_excerpt(json["body"].as_str().unwrap());
    assert_eq!(
        excerpt,
        "Four new systems: Sony PlayStation 2, Atari 2600, Atari 7800 and Atari Lynx (20 \
         systems in all). They pass automated tests and ran on real games during development, \
         but are Experimental like the other systems until they get manual testing. Rescan \
         your ROM folders once (Settings → ROM folders) to pick up the new file types."
    );
    // The real paragraph fits the cap, so no "…".
    assert!(excerpt.chars().count() <= 400);
}

#[test]
fn long_excerpt_is_cut_at_a_word_with_an_ellipsis() {
    let body = format!("## What's new\n\n{}", "word ".repeat(120));
    let excerpt = notes_excerpt(&body);
    assert!(excerpt.ends_with("word…"), "{excerpt}");
    assert!(excerpt.trim_end_matches('…').chars().count() <= 400);
}

#[test]
fn excerpt_without_whats_new_uses_the_first_prose_paragraph() {
    let body = "# Title\n\n> tagline\n\n| a | b |\n\nSee [the guide](https://x.test/g) and \
                `code`\nnext line.\n\nSecond paragraph.";
    assert_eq!(notes_excerpt(body), "See the guide and code next line.");
}

#[test]
fn excerpt_strips_list_markers_and_keeps_plain_brackets() {
    let body = "## What's new in 1.0\n- **Bold** item [x] here\n* __two__\n\nlater";
    assert_eq!(notes_excerpt(body), "Bold item [x] here two");
}

#[test]
fn empty_body_gives_an_empty_excerpt() {
    assert_eq!(notes_excerpt(""), "");
    assert_eq!(notes_excerpt("# Only a heading\n\n> quote"), "");
}

#[test]
fn short_excerpt_is_not_cut() {
    let body = "## What's new\n\nShort text.";
    assert_eq!(notes_excerpt(body), "Short text.");
}

#[test]
fn release_url_allowlist() {
    let good = "https://github.com/vivusk/nostyra/releases/tag/v0.4.0";
    assert_eq!(safe_release_url(good), good);
    for bad in [
        "https://github.com/vivusk/nostyra.evil/releases/tag/v0.4.0",
        "http://github.com/vivusk/nostyra/releases/tag/v0.4.0",
        "https://evil.test/https://github.com/vivusk/nostyra/releases/",
        "",
    ] {
        assert_eq!(safe_release_url(bad), RELEASES_LATEST_URL, "{bad:?}");
    }
}
