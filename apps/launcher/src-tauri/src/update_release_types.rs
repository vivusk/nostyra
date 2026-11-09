//! Release feed result types: the cached release the settings and UI use, the
//! fetch outcome (200 / 304) and the fetch error with its log class.

use http_download_util::DownloadError;
use serde::{Deserialize, Serialize};

/// What the launcher keeps of a release (settings key `update_check_latest`, JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedRelease {
    /// Semver without the leading `v` (`0.4.0`).
    pub version: String,
    pub title: String,
    /// GitHub release page (`html_url`); allowlisted again before opening.
    pub url: String,
    pub published_at: Option<String>,
    /// Plain-text notes excerpt (≤ 400 chars + "…").
    pub excerpt: String,
    /// The Windows portable zip for this version is attached.
    pub has_portable_zip: bool,
}

#[derive(Debug)]
pub enum FeedOutcome {
    /// 200: the release and the `ETag` to send next time.
    Fresh {
        release: CachedRelease,
        etag: Option<String>,
    },
    /// 304: the cached release is still current.
    NotModified,
}

#[derive(Debug)]
pub enum FeedError {
    /// Transport / HTTP status failure (offline, DNS, 403 rate limit, …).
    Http(DownloadError),
    /// Not JSON, required field missing, or a tag that is not semver.
    BadFeed(String),
}

impl std::fmt::Display for FeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(err) => err.fmt(f),
            Self::BadFeed(why) => write!(f, "bad feed: {why}"),
        }
    }
}

impl FeedError {
    /// Short class for the `update check failed class=…` log line.
    pub fn class(&self) -> &'static str {
        match self {
            Self::BadFeed(_) => "bad_feed",
            Self::Http(DownloadError::NotFound) => "not_found",
            Self::Http(DownloadError::Transient(_)) => "transient",
            Self::Http(DownloadError::RateLimited { .. }) => "rate_limited",
            Self::Http(DownloadError::Permanent(_)) => "permanent",
            Self::Http(DownloadError::TooLarge { .. }) => "too_large",
            Self::Http(DownloadError::Cancelled) => "cancelled",
            Self::Http(DownloadError::Io { .. }) => "io",
        }
    }
}
