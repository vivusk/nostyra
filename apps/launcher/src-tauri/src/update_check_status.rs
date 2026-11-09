//! Update status shown by Settings → Updates and the banner: built from settings +
//! cache only (no network), plus the pure rules it uses (running version, "update
//! available", why the automatic check is skipped) so they can be unit-tested.

use std::sync::OnceLock;

use semver::Version;
use serde::Serialize;
use storage_db::{Connection, StorageResult};

use crate::update_check_settings as settings;
use crate::update_release_feed::CachedRelease;
use crate::update_release_rules::{is_newer, parse_release_version};

/// Local override of the running version (manual checks of the update path).
const CURRENT_VERSION_ENV: &str = "NOSTYRA_UPDATE_CURRENT_VERSION";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current_version: String,
    pub enabled: bool,
    pub last_checked_at: Option<i64>,
    pub latest: Option<CachedRelease>,
    /// Latest is newer than the running version and has the portable zip. The skip is
    /// applied by the UI (Settings still shows a skipped update).
    pub update_available: bool,
    pub skipped_version: Option<String>,
    /// Only `check_for_updates` sets this; the automatic path stays silent.
    pub check_failed: bool,
}

/// Status from settings + cache only (no network).
pub fn build_status(conn: &Connection, check_failed: bool) -> StorageResult<UpdateStatus> {
    let current = current_version();
    let latest = settings::load_update_cache(conn)?.map(|(release, _)| release);
    Ok(UpdateStatus {
        current_version: current.to_string(),
        enabled: settings::update_check_enabled(conn)?,
        last_checked_at: settings::last_checked_at(conn)?,
        update_available: is_update_available(latest.as_ref(), current),
        latest,
        skipped_version: settings::skipped_version(conn)?,
        check_failed,
    })
}

/// The cached release is newer than `current` and ships the Windows portable zip.
pub fn is_update_available(latest: Option<&CachedRelease>, current: &Version) -> bool {
    latest.is_some_and(|release| {
        release.has_portable_zip
            && parse_release_version(&release.version).is_some_and(|v| is_newer(&v, current))
    })
}

/// Why the automatic check does not run, in priority order: turned off, debug build,
/// checked less than a day ago. `None` = run it.
pub fn automatic_skip_reason(enabled: bool, debug_build: bool, due: bool) -> Option<&'static str> {
    if !enabled {
        Some("disabled")
    } else if debug_build {
        Some("debug")
    } else if !due {
        Some("not_due")
    } else {
        None
    }
}

/// `CARGO_PKG_VERSION`, or `NOSTYRA_UPDATE_CURRENT_VERSION` when set (cached once).
fn current_version() -> &'static Version {
    static CURRENT: OnceLock<Version> = OnceLock::new();
    CURRENT.get_or_init(|| resolve_current_version(std::env::var(CURRENT_VERSION_ENV).ok()))
}

/// The override (`0.3.0` or `v0.3.0`) wins when it parses; an unparseable value is
/// logged and ignored. The workspace version is always semver (release.ps1 writes x.y.z).
pub fn resolve_current_version(env_value: Option<String>) -> Version {
    if let Some(raw) = env_value.filter(|v| !v.trim().is_empty()) {
        match parse_release_version(&raw) {
            Some(version) => {
                tracing::info!(%version, "update check uses {CURRENT_VERSION_ENV}");
                return version;
            }
            None => tracing::warn!(value = %raw, "{CURRENT_VERSION_ENV} is not semver; ignored"),
        }
    }
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0))
}

#[cfg(test)]
#[path = "update_check_status_tests.rs"]
mod tests;
