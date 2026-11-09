//! One conditional GET of the GitHub "latest release" endpoint through the shared
//! launcher client, parsed into the small `CachedRelease` the settings cache and the UI
//! use. Unknown JSON fields are ignored; only `tag_name` and `html_url` are required.

use std::sync::atomic::AtomicBool;

use http_download_util::{read_body_with_limit, HttpClient};
use serde::Deserialize;

use crate::update_release_rules::{
    has_portable_zip, notes_excerpt, parse_release_version, RELEASE_FEED_URL,
};
pub use crate::update_release_types::{CachedRelease, FeedError, FeedOutcome};

/// The answer is ~25 KB today; anything above 1 MiB is not a release.
const FEED_BODY_LIMIT: u64 = 1024 * 1024;

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
}

/// GETs the latest release. `etag` (from a valid cached release only) makes the
/// request conditional; GitHub does not count 304 answers against the rate limit.
pub fn fetch_latest_release(
    http: &HttpClient,
    etag: Option<&str>,
) -> Result<FeedOutcome, FeedError> {
    let mut headers = vec![
        ("Accept", "application/vnd.github+json"),
        ("X-GitHub-Api-Version", "2022-11-28"),
    ];
    if let Some(etag) = etag {
        headers.push(("If-None-Match", etag));
    }
    let response = http
        .get(RELEASE_FEED_URL, &headers)
        .map_err(FeedError::Http)?;
    if response.status() == 304 {
        return Ok(FeedOutcome::NotModified);
    }
    let new_etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let never_cancelled = AtomicBool::new(false);
    let body = read_body_with_limit(response, FEED_BODY_LIMIT, &never_cancelled)
        .map_err(FeedError::Http)?;
    Ok(FeedOutcome::Fresh {
        release: parse_release_json(&body)?,
        etag: new_etag,
    })
}

/// Feed JSON → `CachedRelease` (version normalised, excerpt and zip rule applied).
pub fn parse_release_json(bytes: &[u8]) -> Result<CachedRelease, FeedError> {
    let release: GithubRelease =
        serde_json::from_slice(bytes).map_err(|err| FeedError::BadFeed(err.to_string()))?;
    let version = parse_release_version(&release.tag_name)
        .ok_or_else(|| FeedError::BadFeed(format!("tag {:?} is not semver", release.tag_name)))?;
    let names = release.assets.iter().map(|asset| asset.name.as_str());
    Ok(CachedRelease {
        has_portable_zip: has_portable_zip(names, &version),
        title: release
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("Nostyra {version}")),
        version: version.to_string(),
        url: release.html_url,
        published_at: release.published_at,
        excerpt: notes_excerpt(release.body.as_deref().unwrap_or("")),
    })
}

#[cfg(test)]
#[path = "update_release_feed_tests.rs"]
pub(crate) mod tests;
