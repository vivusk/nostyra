//! One thumbnail file → bytes, with the retry policy: primary host first,
//! mirror on a 404 or a transient failure, at most 3 retries with
//! 500 ms·2ⁿ + jitter backoff (or the server's `Retry-After`). `cancel` is
//! checked before every attempt and polled every 100 ms while backing off,
//! so a cancel lands within one in-flight request timeout.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use http_download_util::{read_body_with_limit, DownloadError, HttpClient};

/// Largest thumbnail accepted (libretro box art is ~100–400 KB).
pub(crate) const MAX_ART_BYTES: u64 = 8 * 1024 * 1024;
/// Retries after the first attempt, across both hosts.
const MAX_RETRIES: u32 = 3;
const BACKOFF_BASE: Duration = Duration::from_millis(500);
const MAX_JITTER_MS: u32 = 250;
/// Cancel poll interval while sleeping a backoff.
const CANCEL_POLL: Duration = Duration::from_millis(100);

/// Downloads the first URL of `urls` (primary, then mirrors) that serves the
/// file. `NotFound` only when the last URL answered 404; other errors after
/// the retries are spent are returned as classified by `http-download-util`.
pub(crate) fn download_thumbnail(
    http: &HttpClient,
    urls: &[String],
    cancel: &AtomicBool,
) -> Result<Vec<u8>, DownloadError> {
    let mut url_index = 0;
    let mut retries = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(DownloadError::Cancelled);
        }
        let Some(url) = urls.get(url_index) else {
            return Err(DownloadError::NotFound);
        };
        let error = match http
            .get(url, &[])
            .and_then(|response| read_body_with_limit(response, MAX_ART_BYTES, cancel))
        {
            Ok(bytes) => return Ok(bytes),
            Err(error) => error,
        };
        match error {
            // The mirror is the synced source of truth: only its 404 is final.
            DownloadError::NotFound if url_index + 1 < urls.len() => url_index += 1,
            // Offline is deterministic: retrying cannot help.
            error if error.is_transient() && !http.is_offline() && retries < MAX_RETRIES => {
                retries += 1;
                tracing::debug!(url, %error, retries, "thumbnail download retry");
                url_index = (url_index + 1) % urls.len();
                if !sleep_unless_cancelled(backoff(retries, &error), cancel) {
                    return Err(DownloadError::Cancelled);
                }
            }
            error => return Err(error),
        }
    }
}

/// Wait before retry `retry` (1-based): the server's `Retry-After` when it
/// sent one, else 500 ms·2ⁿ⁻¹ plus up to 250 ms of jitter so four workers do
/// not retry in lockstep.
fn backoff(retry: u32, error: &DownloadError) -> Duration {
    if let DownloadError::RateLimited { retry_after, .. } = error {
        return *retry_after;
    }
    let jitter = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |now| now.subsec_nanos() % (MAX_JITTER_MS * 1_000_000));
    BACKOFF_BASE * 2u32.pow(retry - 1) + Duration::from_nanos(u64::from(jitter))
}

/// Sleeps `duration` in short slices; false as soon as `cancel` is set.
pub(crate) fn sleep_unless_cancelled(duration: Duration, cancel: &AtomicBool) -> bool {
    let mut left = duration;
    while !left.is_zero() {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        let slice = left.min(CANCEL_POLL);
        std::thread::sleep(slice);
        left -= slice;
    }
    !cancel.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_honours_retry_after() {
        let transient = DownloadError::Transient("t".into());
        assert!(backoff(1, &transient) >= BACKOFF_BASE);
        assert!(backoff(3, &transient) >= BACKOFF_BASE * 4);
        let limited = DownloadError::RateLimited {
            status: 429,
            retry_after: Duration::from_secs(2),
        };
        assert_eq!(backoff(1, &limited), Duration::from_secs(2));
    }

    #[test]
    fn cancel_interrupts_backoff_quickly() {
        let cancel = AtomicBool::new(false);
        let started = std::time::Instant::now();
        let cancelled_at = std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(150));
                cancel.store(true, Ordering::Relaxed);
            });
            assert!(!sleep_unless_cancelled(Duration::from_secs(5), &cancel));
            started.elapsed()
        });
        // Cancel lands within one poll slice (+ scheduling slack).
        assert!(cancelled_at < Duration::from_millis(150) + CANCEL_POLL * 2);
    }

    #[test]
    fn offline_client_fails_once_without_retry() {
        let http = http_download_util::build_http_client("Nostyra-test", true);
        let started = std::time::Instant::now();
        let result = download_thumbnail(
            &http,
            &["https://a.invalid/x.png".into()],
            &AtomicBool::new(false),
        );
        assert!(matches!(result, Err(DownloadError::Transient(_))));
        assert!(started.elapsed() < BACKOFF_BASE);
    }
}
