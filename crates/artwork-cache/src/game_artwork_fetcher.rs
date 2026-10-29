//! Per-game orchestration: name candidates → index lookup (or direct GET) →
//! download → validate/re-encode → atomic write, reported per artwork kind
//! so the launcher can persist `cached` / `not-found` / `failed` and skip
//! `Transient` (retried by the next job instead of waiting out a retry window).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use http_download_util::{DownloadError, HttpClient};

use crate::artwork_downloader::download_thumbnail;
use crate::artwork_file_store::write_atomically;
use crate::thumbnail_source_urls::thumbnail_urls;
use crate::{
    encode_cover_jpeg, resolve_names, validate_hero_png, ArtworkKind, ArtworkOutcome,
    GameArtRequest, KindOutcome, ThumbnailFolder, ThumbnailIndex,
};

/// Fetches the requested kinds for one game into `artwork_dir`. With an
/// `index`, only names it lists are downloaded; without one, each candidate
/// name is tried directly (404 → next). The first `Transient` or `Cancelled`
/// kind ends the game: remaining kinds get the same outcome, so a dead
/// network costs one retry chain per game. Never panics, never leaves a tmp
/// file; `Transient("offline")` comes back instantly for an offline client.
pub fn fetch_game_artwork(
    http: &HttpClient,
    index: Option<&ThumbnailIndex>,
    request: &GameArtRequest,
    artwork_dir: &Path,
    cancel: &AtomicBool,
) -> ArtworkOutcome {
    let candidates = resolve_names(&request.no_intro_name);
    let mut outcome = ArtworkOutcome::default();
    let mut stop: Option<KindOutcome> = None;
    for &kind in &request.kinds {
        if outcome.get(kind).is_some() {
            continue;
        }
        let result = match &stop {
            Some(same) => same.clone(),
            None => fetch_kind(
                http,
                index,
                request.system,
                kind,
                &candidates,
                request.game_id,
                artwork_dir,
                cancel,
                &mut outcome.network,
            ),
        };
        if matches!(result, KindOutcome::Transient(_) | KindOutcome::Cancelled) {
            stop.get_or_insert_with(|| result.clone());
        }
        tracing::debug!(
            game_id = request.game_id,
            kind = kind.as_str(),
            ?result,
            "artwork fetched"
        );
        outcome.results.push((kind, result));
    }
    outcome
}

#[allow(clippy::too_many_arguments)] // private helper; a struct would only rename the same fields
fn fetch_kind(
    http: &HttpClient,
    index: Option<&ThumbnailIndex>,
    system: &system_catalog::SystemSpec,
    kind: ArtworkKind,
    candidates: &[String],
    game_id: i64,
    artwork_dir: &Path,
    cancel: &AtomicBool,
    network: &mut bool,
) -> KindOutcome {
    for &folder in kind.folders() {
        let names: Vec<&str> = match index {
            Some(index) => index.find_name(folder, candidates).into_iter().collect(),
            None => candidates.iter().map(String::as_str).collect(),
        };
        for name in names {
            if cancel.load(Ordering::Relaxed) {
                return KindOutcome::Cancelled;
            }
            match download_thumbnail(http, &thumbnail_urls(system, folder, name), cancel) {
                Ok(bytes) => {
                    *network = true;
                    return store(kind, game_id, folder, name, &bytes, artwork_dir);
                }
                // Index lag or a direct-GET miss: try the next name / folder.
                Err(DownloadError::NotFound) => *network = true,
                Err(DownloadError::Cancelled) => return KindOutcome::Cancelled,
                Err(DownloadError::Transient(reason)) => return KindOutcome::Transient(reason),
                Err(error) if error.is_transient() => {
                    return KindOutcome::Transient(error.to_string())
                }
                Err(error) => {
                    *network = true;
                    return KindOutcome::Failed(error.to_string());
                }
            }
        }
    }
    KindOutcome::NotFound
}

/// Validates / re-encodes `bytes` and writes the kind's cache file.
fn store(
    kind: ArtworkKind,
    game_id: i64,
    folder: ThumbnailFolder,
    name: &str,
    bytes: &[u8],
    artwork_dir: &Path,
) -> KindOutcome {
    let relative_path = kind.file_name(game_id);
    let written = match kind {
        ArtworkKind::Cover => encode_cover_jpeg(bytes)
            .and_then(|jpeg| write_atomically(artwork_dir, &relative_path, &jpeg)),
        ArtworkKind::Hero => validate_hero_png(bytes)
            .and_then(|()| write_atomically(artwork_dir, &relative_path, bytes)),
    };
    match written {
        Ok(()) => KindOutcome::Cached {
            relative_path,
            source_name: format!("{}/{name}.png", folder.dir_name()),
        },
        Err(error) => KindOutcome::Failed(error.to_string()),
    }
}
