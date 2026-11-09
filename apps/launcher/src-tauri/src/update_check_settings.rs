//! Update-checker state in the key/value `settings` table: the startup toggle, the
//! last successful check, the cached release + its `ETag`, and the skipped version.
//! Desktop-only keys; the web edition never reads them.

use storage_db::{Connection, StorageResult};

use crate::update_release_feed::CachedRelease;
use crate::update_release_rules::{parse_release_version, CHECK_INTERVAL_SECS};

/// Automatic check at startup; on unless exactly "0".
pub const UPDATE_CHECK_ENABLED: &str = "update_check_enabled";
/// Unix seconds of the last successful check (200 or 304).
pub const UPDATE_CHECK_LAST_AT: &str = "update_check_last_at";
/// `ETag` of the cached answer ("" = none).
pub const UPDATE_CHECK_ETAG: &str = "update_check_etag";
/// JSON `CachedRelease` of the last 200 answer.
pub const UPDATE_CHECK_LATEST: &str = "update_check_latest";
/// Exact version the user chose to skip (banner hidden for it only).
pub const UPDATE_SKIPPED_VERSION: &str = "update_skipped_version";

pub fn update_check_enabled(conn: &Connection) -> StorageResult<bool> {
    Ok(storage_db::get_setting(conn, UPDATE_CHECK_ENABLED)?.as_deref() != Some("0"))
}

pub fn set_update_check_enabled(conn: &Connection, enabled: bool) -> StorageResult<()> {
    storage_db::set_setting(conn, UPDATE_CHECK_ENABLED, if enabled { "1" } else { "0" })
}

/// Last successful check; unparsable values count as never.
pub fn last_checked_at(conn: &Connection) -> StorageResult<Option<i64>> {
    Ok(storage_db::get_setting(conn, UPDATE_CHECK_LAST_AT)?.and_then(|v| v.parse().ok()))
}

/// Due when never checked, the last success is ≥ 24 h old, or the stamp lies in the
/// future (clock skew must not silence the checker for good).
pub fn is_due(now: i64, last_at: Option<i64>) -> bool {
    match last_at {
        None => true,
        Some(last) if last > now => true,
        Some(last) => now - last >= CHECK_INTERVAL_SECS,
    }
}

/// The cached release and its `ETag`. Missing or corrupt JSON → `None` (warned when
/// corrupt) and the stale `ETag` is cleared, so the next request is unconditional and
/// a 304 can never stand in for a release we no longer have.
pub fn load_update_cache(
    conn: &Connection,
) -> StorageResult<Option<(CachedRelease, Option<String>)>> {
    let json = storage_db::get_setting(conn, UPDATE_CHECK_LATEST)?;
    let release = json
        .as_deref()
        .and_then(|text| match serde_json::from_str(text) {
            Ok(release) => Some(release),
            Err(err) => {
                tracing::warn!(%err, "cached update release is corrupt; ignored");
                None
            }
        });
    let etag = storage_db::get_setting(conn, UPDATE_CHECK_ETAG)?.filter(|etag| !etag.is_empty());
    let Some(release) = release else {
        if etag.is_some() {
            storage_db::set_setting(conn, UPDATE_CHECK_ETAG, "")?;
        }
        return Ok(None);
    };
    Ok(Some((release, etag)))
}

/// 200 answer: cache the release and its `ETag`, stamp the check.
pub fn store_fresh(
    conn: &Connection,
    release: &CachedRelease,
    etag: Option<&str>,
    now: i64,
) -> StorageResult<()> {
    let json = serde_json::to_string(release)
        .map_err(|err| storage_db::StorageError::InvalidInput(err.to_string()))?;
    storage_db::set_setting(conn, UPDATE_CHECK_LATEST, &json)?;
    storage_db::set_setting(conn, UPDATE_CHECK_ETAG, etag.unwrap_or(""))?;
    stamp_checked(conn, now)
}

/// 304 answer: the cache stays, only the check time moves.
pub fn stamp_checked(conn: &Connection, now: i64) -> StorageResult<()> {
    storage_db::set_setting(conn, UPDATE_CHECK_LAST_AT, &now.to_string())
}

pub fn skipped_version(conn: &Connection) -> StorageResult<Option<String>> {
    Ok(storage_db::get_setting(conn, UPDATE_SKIPPED_VERSION)?.filter(|v| !v.is_empty()))
}

/// Stores `version` (normalised, no `v`) when it is semver; returns false otherwise.
pub fn set_skipped_version(conn: &Connection, version: &str) -> StorageResult<bool> {
    let Some(parsed) = parse_release_version(version) else {
        return Ok(false);
    };
    storage_db::set_setting(conn, UPDATE_SKIPPED_VERSION, &parsed.to_string())?;
    Ok(true)
}

#[cfg(test)]
#[path = "update_check_settings_tests.rs"]
mod tests;
