//! Toggle, throttle, cache round trip, stale-etag cleanup and skip rules against an
//! in-memory database (same migrations as the real one).

use storage_db::open_in_memory_database;

use super::*;

const NOW: i64 = 1_800_000_000;
const HOUR: i64 = 3_600;

fn release(version: &str) -> CachedRelease {
    CachedRelease {
        version: version.into(),
        title: format!("Nostyra {version}"),
        url: format!("https://github.com/vivusk/nostyra/releases/tag/v{version}"),
        published_at: Some("2026-11-09T01:33:21Z".into()),
        excerpt: "Four new systems.".into(),
        has_portable_zip: true,
    }
}

#[test]
fn enabled_by_default_and_off_only_for_zero() {
    let conn = open_in_memory_database().unwrap();
    assert!(update_check_enabled(&conn).unwrap());
    set_update_check_enabled(&conn, false).unwrap();
    assert!(!update_check_enabled(&conn).unwrap());
    set_update_check_enabled(&conn, true).unwrap();
    assert!(update_check_enabled(&conn).unwrap());
}

#[test]
fn due_rules() {
    assert!(is_due(NOW, None));
    assert!(!is_due(NOW, Some(NOW - 23 * HOUR)));
    assert!(is_due(NOW, Some(NOW - 24 * HOUR)));
    assert!(is_due(NOW, Some(NOW + HOUR)), "future stamp counts as due");
}

#[test]
fn fresh_answer_round_trips() {
    let conn = open_in_memory_database().unwrap();
    assert_eq!(load_update_cache(&conn).unwrap(), None);
    assert_eq!(last_checked_at(&conn).unwrap(), None);
    store_fresh(&conn, &release("0.4.0"), Some("W/\"abc\""), NOW).unwrap();
    let (cached, etag) = load_update_cache(&conn).unwrap().unwrap();
    assert_eq!(cached, release("0.4.0"));
    assert_eq!(etag.as_deref(), Some("W/\"abc\""));
    assert_eq!(last_checked_at(&conn).unwrap(), Some(NOW));
}

#[test]
fn fresh_answer_without_etag_stores_none() {
    let conn = open_in_memory_database().unwrap();
    store_fresh(&conn, &release("0.4.0"), Some("old"), NOW).unwrap();
    store_fresh(&conn, &release("0.4.1"), None, NOW).unwrap();
    let (cached, etag) = load_update_cache(&conn).unwrap().unwrap();
    assert_eq!(cached.version, "0.4.1");
    assert_eq!(etag, None);
}

#[test]
fn not_modified_stamp_keeps_the_cached_release() {
    let conn = open_in_memory_database().unwrap();
    store_fresh(&conn, &release("0.4.0"), Some("e1"), NOW).unwrap();
    stamp_checked(&conn, NOW + 25 * HOUR).unwrap();
    let (cached, etag) = load_update_cache(&conn).unwrap().unwrap();
    assert_eq!(cached, release("0.4.0"));
    assert_eq!(etag.as_deref(), Some("e1"));
    assert_eq!(last_checked_at(&conn).unwrap(), Some(NOW + 25 * HOUR));
}

#[test]
fn corrupt_cache_returns_none_and_clears_the_etag() {
    let conn = open_in_memory_database().unwrap();
    store_fresh(&conn, &release("0.4.0"), Some("e1"), NOW).unwrap();
    storage_db::set_setting(&conn, UPDATE_CHECK_LATEST, "{not json").unwrap();
    assert_eq!(load_update_cache(&conn).unwrap(), None);
    let etag = storage_db::get_setting(&conn, UPDATE_CHECK_ETAG).unwrap();
    assert_eq!(etag.as_deref(), Some(""));
}

#[test]
fn etag_without_any_release_is_cleared() {
    let conn = open_in_memory_database().unwrap();
    storage_db::set_setting(&conn, UPDATE_CHECK_ETAG, "orphan").unwrap();
    assert_eq!(load_update_cache(&conn).unwrap(), None);
    let etag = storage_db::get_setting(&conn, UPDATE_CHECK_ETAG).unwrap();
    assert_eq!(etag.as_deref(), Some(""));
}

#[test]
fn skipped_version_is_stored_only_when_semver() {
    let conn = open_in_memory_database().unwrap();
    assert_eq!(skipped_version(&conn).unwrap(), None);
    assert!(!set_skipped_version(&conn, "latest").unwrap());
    assert!(!set_skipped_version(&conn, "0.5").unwrap());
    assert_eq!(skipped_version(&conn).unwrap(), None);
    assert!(set_skipped_version(&conn, "v0.4.1").unwrap());
    assert_eq!(skipped_version(&conn).unwrap().as_deref(), Some("0.4.1"));
}

#[test]
fn bad_last_at_counts_as_never() {
    let conn = open_in_memory_database().unwrap();
    storage_db::set_setting(&conn, UPDATE_CHECK_LAST_AT, "yesterday").unwrap();
    assert_eq!(last_checked_at(&conn).unwrap(), None);
}
