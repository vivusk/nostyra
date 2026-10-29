//! Loads the thumbnail index from its disk cache and refreshes it from the
//! GitHub tree API.
//!
//! Refresh rule (single source of truth): cache file mtime younger than
//! [`INDEX_MAX_AGE`] → used as-is, no network. Older or missing → one
//! conditional GET (`If-None-Match` from the `.etag` sidecar; 304 touches the
//! mtime), at most once per system per process (the API allows 60
//! unauthenticated requests per hour). One cache per system:
//! `index/<system id>-tree.json` (+ `.etag`). Any failure falls back to the stale cache, else `None`
//! (the caller then GETs candidate files directly, 404 = not found).

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use http_download_util::{read_body_with_limit, DownloadError, HttpClient};
use system_catalog::SystemSpec;

use crate::artwork_file_store::{write_atomically, INDEX_DIR};
use crate::thumbnail_source_urls::index_url;
use crate::{ArtworkError, ThumbnailFolder, ThumbnailIndex};

/// Age under which the cached index is trusted without asking GitHub.
pub const INDEX_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// A system's tree JSON is a few MB (SNES ~5 MB); anything far beyond is not the tree.
const MAX_INDEX_BYTES: u64 = 32 * 1024 * 1024;

/// Systems whose index this process already asked GitHub for (success or not).
static NETWORK_ATTEMPTED: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);

/// `index/<id>-tree.json` and its `.etag` sidecar.
fn index_file_names(system: &SystemSpec) -> (String, String) {
    let json = format!("{}-tree.json", system.id);
    let etag = format!("{json}.etag");
    (json, etag)
}

/// True the first time `system` asks for the network in this process.
fn first_network_attempt(system: &'static SystemSpec) -> bool {
    let mut attempted = NETWORK_ATTEMPTED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    attempted.get_or_insert_with(HashSet::new).insert(system.id)
}

/// True when a cache written at `modified` may still be used at `now`.
/// A modification time in the future (clock moved back) counts as fresh.
pub fn index_cache_is_fresh(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .map_or(true, |age| age < INDEX_MAX_AGE)
}

impl ThumbnailIndex {
    /// Loads `system`'s index from `<artwork_dir>/index/`, refreshing it per the
    /// module's rule. `now` is injected so the age rule is testable. Never
    /// fails hard: `None` means "no index, fetch candidates directly".
    pub fn load_or_fetch(
        http: &HttpClient,
        artwork_dir: &Path,
        system: &'static SystemSpec,
        now: SystemTime,
    ) -> Option<Self> {
        let index_dir = artwork_dir.join(INDEX_DIR);
        let json_path = index_dir.join(index_file_names(system).0);
        let modified = std::fs::metadata(&json_path)
            .and_then(|meta| meta.modified())
            .ok();
        if modified.is_some_and(|time| index_cache_is_fresh(time, now)) {
            if let Some(index) = read_cached(&json_path) {
                return Some(index);
            }
        }
        if first_network_attempt(system) {
            match fetch_index(http, &index_dir, system, modified.is_some()) {
                Ok(Some(index)) => return Some(index),
                Ok(None) => touch(&json_path, now),
                Err(error) => tracing::warn!(%error, "thumbnail index refresh failed"),
            }
        }
        read_cached(&json_path)
    }
}

/// One (conditional) GET of the tree. `Ok(None)` = 304, cache still valid.
/// A fresh body is parsed before it replaces the cache.
fn fetch_index(
    http: &HttpClient,
    index_dir: &Path,
    system: &SystemSpec,
    have_cache: bool,
) -> Result<Option<ThumbnailIndex>, ArtworkError> {
    let (index_file, etag_file) = index_file_names(system);
    let etag = have_cache
        .then(|| std::fs::read_to_string(index_dir.join(&etag_file)).ok())
        .flatten();
    let mut headers = vec![("Accept", "application/vnd.github+json")];
    if let Some(etag) = etag.as_deref() {
        headers.push(("If-None-Match", etag.trim()));
    }
    let network =
        |error: DownloadError| ArtworkError::InvalidIndex(format!("download failed: {error}"));
    let response = http.get(&index_url(system), &headers).map_err(network)?;
    if response.status().as_u16() == 304 {
        tracing::info!("thumbnail index not modified");
        return Ok(None);
    }
    let new_etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = read_body_with_limit(response, MAX_INDEX_BYTES, &AtomicBool::new(false))
        .map_err(network)?;
    let index = ThumbnailIndex::parse_tree_json(&bytes)?;
    write_atomically(index_dir, &index_file, &bytes)?;
    match new_etag {
        Some(etag) => write_atomically(index_dir, &etag_file, etag.as_bytes())?,
        None => {
            let _ = std::fs::remove_file(index_dir.join(&etag_file));
        }
    }
    tracing::info!(
        system = system.id,
        boxarts = index.len(ThumbnailFolder::Boxarts),
        "thumbnail index downloaded"
    );
    Ok(Some(index))
}

fn read_cached(json_path: &Path) -> Option<ThumbnailIndex> {
    let bytes = std::fs::read(json_path).ok()?;
    ThumbnailIndex::parse_tree_json(&bytes)
        .inspect_err(|error| tracing::warn!(%error, "cached thumbnail index unreadable"))
        .ok()
}

/// Marks the cache as confirmed at `now` (304), restarting its 24 h window.
fn touch(json_path: &Path, now: SystemTime) {
    let result = std::fs::File::options()
        .write(true)
        .open(json_path)
        .and_then(|file| file.set_modified(now));
    if let Err(error) = result {
        tracing::warn!(%error, "cannot refresh thumbnail index mtime");
    }
}
