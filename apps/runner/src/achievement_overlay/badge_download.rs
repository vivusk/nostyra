//! The worker side of the badge cache: one GET per missing image (runs on the
//! achievement HTTP worker as a background job), refused unless it decodes as an
//! image (a proxy's HTML error page never lands in the shared cache), written
//! atomically so the launcher never reads half a file. Plus the decoder both sides use.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use achievements::TransportJob;

use super::badge_cache::{BadgeImage, BadgeKey, BADGE_SIZE};

/// RA badges are a few KiB; anything bigger is not a badge.
const MAX_BADGE_BYTES: u64 = 1024 * 1024;

/// Downloads `url` to `path`, then sends `key` on `landed` (failures are logged only:
/// the placeholder stays, the next session retries).
pub fn download_job(
    url: String,
    path: PathBuf,
    key: BadgeKey,
    landed: Sender<BadgeKey>,
) -> TransportJob {
    Box::new(move |transport| {
        let bytes = match transport.download(&url, MAX_BADGE_BYTES) {
            Ok(bytes) => bytes,
            Err(reason) => {
                tracing::debug!(%url, %reason, "achievement badge not downloaded");
                return;
            }
        };
        if image::load_from_memory(&bytes).is_err() {
            tracing::warn!(%url, "achievement badge download is not an image; skipped");
            return;
        }
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| config_layers::write_atomically(&path, &bytes));
        match written {
            Ok(()) => drop(landed.send(key)),
            Err(err) => tracing::warn!(path = %path.display(), %err, "cannot cache badge"),
        }
    })
}

/// PNG / JPEG bytes → [`BADGE_SIZE`]² RGBA8 (resized when the server sent another size).
pub fn decode_badge(bytes: &[u8]) -> Option<BadgeImage> {
    let rgba = image::load_from_memory(bytes).ok()?.to_rgba8();
    let rgba = if rgba.dimensions() == (BADGE_SIZE, BADGE_SIZE) {
        rgba
    } else {
        let filter = image::imageops::FilterType::Triangle;
        image::imageops::resize(&rgba, BADGE_SIZE, BADGE_SIZE, filter)
    };
    Some(BadgeImage {
        rgba: rgba.into_raw(),
    })
}
