//! The HTTP seam of the cloud providers: [`HttpTransport`] over the shared
//! `http_download_util::HttpClient` (live) or `FakeHttpTransport` (scripted tests).
//! Plus the shared bits every REST provider needs: timeouts, transport / status
//! → [`CloudError`] mapping, bounded body reads, form POST and a retry wrapper.

use std::io::Read;
use std::time::{Duration, Instant, SystemTime};

use http_download_util::{
    parse_retry_after, retry_with_backoff, DownloadError, HttpClient, HttpMethod, HttpReply,
    HttpRequest, RequestBody, RetryClass, RetryHint, RetryPolicy,
};

use crate::CloudError;

/// Research defaults (phase 00 confirms / adjusts in 04–06).
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// Background sync request bound (one chunk / one metadata call).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Pre-play pull request bound (15 s budget overall).
pub const PRE_PLAY_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Upload chunk: 5 MiB = 20 × 256 KiB (Google) = 16 × 320 KiB (Microsoft).
pub const UPLOAD_CHUNK_BYTES: u64 = 5 * 1024 * 1024;
/// Slowest link a media transfer (download range / upload chunk) is sized for:
/// 64 KiB/s ≈ 0.5 Mbit/s, so a 5 MiB piece gets 80 s instead of [`REQUEST_TIMEOUT`].
pub const MIN_TRANSFER_BYTES_PER_SEC: u64 = 64 * 1024;
/// Cap for JSON metadata / token replies read into memory.
pub const MAX_JSON_REPLY_BYTES: u64 = 4 * 1024 * 1024;

/// Time limits of one logical call: per-request timeout plus an optional retry
/// deadline (the 15 s pre-play budget). Threaded through token refresh so the first
/// call of a pre-play pull cannot outlast the budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestBudget {
    pub timeout: Duration,
    pub deadline: Option<Instant>,
}

impl RequestBudget {
    /// Background sync: [`REQUEST_TIMEOUT`] per request, no deadline.
    pub const BACKGROUND: Self = Self {
        timeout: REQUEST_TIMEOUT,
        deadline: None,
    };

    /// Pre-play pull (or best-effort sign-out): [`PRE_PLAY_REQUEST_TIMEOUT`] per
    /// request, nothing runs or sleeps past `deadline`.
    pub fn until(deadline: Instant) -> Self {
        Self {
            timeout: PRE_PLAY_REQUEST_TIMEOUT,
            deadline: Some(deadline),
        }
    }

    /// Default retry policy capped by the deadline.
    pub fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy {
            deadline: self.deadline,
            ..RetryPolicy::default()
        }
    }

    /// Per-request timeout clipped to what is left; `Offline` once the budget is spent.
    pub fn request_timeout(&self) -> Result<Duration, CloudError> {
        self.transfer_timeout(0)
    }

    /// [`Self::request_timeout`] for a request moving `bytes` of media (the timeout
    /// covers the whole body). Background: at least `bytes` at
    /// [`MIN_TRANSFER_BYTES_PER_SEC`]; pre-play keeps its short per-request bound.
    pub fn transfer_timeout(&self, bytes: u64) -> Result<Duration, CloudError> {
        let Some(deadline) = self.deadline else {
            let sized = Duration::from_secs(bytes.div_ceil(MIN_TRANSFER_BYTES_PER_SEC));
            return Ok(self.timeout.max(sized));
        };
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(CloudError::Offline);
        }
        Ok(self.timeout.min(left))
    }
}

/// Sends one request; the reply comes back for every HTTP status.
pub trait HttpTransport: Send + Sync {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpReply, CloudError>;

    /// True when the network is switched off (`NOSTYRA_OFFLINE=1`): no retries.
    fn is_offline(&self) -> bool {
        false
    }
}

impl HttpTransport for HttpClient {
    fn send(&self, request: HttpRequest<'_>) -> Result<HttpReply, CloudError> {
        self.request(request).map_err(transport_error)
    }

    fn is_offline(&self) -> bool {
        HttpClient::is_offline(self)
    }
}

/// Transport failure → cloud error (no URL / header in the message).
pub fn transport_error(err: DownloadError) -> CloudError {
    match err {
        DownloadError::Transient(_) | DownloadError::Io { .. } => CloudError::Offline,
        DownloadError::RateLimited { retry_after, .. } => CloudError::RateLimited {
            retry_after: Some(retry_after),
        },
        DownloadError::NotFound => CloudError::NotFound,
        other => CloudError::Other(other.to_string()),
    }
}

/// Generic classification of a non-2xx reply (providers refine 400 / 403 / 409).
pub fn status_error(reply: &HttpReply) -> CloudError {
    match reply.status {
        401 => CloudError::AuthExpired,
        404 => CloudError::NotFound,
        412 => CloudError::RevMismatch,
        429 | 503 => CloudError::RateLimited {
            retry_after: retry_after(reply),
        },
        507 => CloudError::QuotaFull,
        500..=599 => CloudError::Offline,
        status => CloudError::Other(format!("http status {status}")),
    }
}

/// The reply's `Retry-After` (seconds or HTTP date), if any.
pub fn retry_after(reply: &HttpReply) -> Option<Duration> {
    reply
        .header("retry-after")
        .and_then(|value| parse_retry_after(value, SystemTime::now()))
}

/// Reads at most `limit` bytes of the body (more → `Other`).
pub fn read_body(reply: HttpReply, limit: u64) -> Result<Vec<u8>, CloudError> {
    let mut body = Vec::new();
    reply
        .body
        .take(limit + 1)
        .read_to_end(&mut body)
        .map_err(|_| CloudError::Offline)?;
    if body.len() as u64 > limit {
        return Err(CloudError::Other(format!("reply over {limit} bytes")));
    }
    Ok(body)
}

/// A form POST's reply, body read into memory.
#[derive(Debug)]
pub struct FormReply {
    pub status: u16,
    pub retry_after: Option<Duration>,
    pub body: Vec<u8>,
}

/// POSTs an `application/x-www-form-urlencoded` body; a [`FormReply`] for every status.
pub fn post_form(
    transport: &dyn HttpTransport,
    url: &str,
    form: &str,
    timeout: Duration,
) -> Result<FormReply, CloudError> {
    let reply = transport.send(HttpRequest {
        method: HttpMethod::Post,
        url,
        headers: &[
            ("Content-Type", "application/x-www-form-urlencoded"),
            ("Accept", "application/json"),
        ],
        body: RequestBody::Bytes(form.as_bytes()),
        connect_timeout: Some(CONNECT_TIMEOUT),
        timeout: Some(timeout),
    })?;
    let status = reply.status;
    let retry_after = retry_after(&reply);
    Ok(FormReply {
        status,
        retry_after,
        body: read_body(reply, MAX_JSON_REPLY_BYTES)?,
    })
}

/// Runs `op` under `policy`; with the network switched off, once and without waits.
pub fn with_retry<T>(
    transport: &dyn HttpTransport,
    policy: RetryPolicy,
    mut op: impl FnMut() -> Result<T, CloudError>,
) -> Result<T, CloudError> {
    if transport.is_offline() {
        return Err(CloudError::Offline);
    }
    retry_with_backoff(policy, |_| op())
}

impl RetryClass for CloudError {
    fn retry_hint(&self) -> RetryHint {
        match self {
            Self::RateLimited {
                retry_after: Some(wait),
            } => RetryHint::RetryAfter(*wait),
            Self::RateLimited { retry_after: None } | Self::Offline => RetryHint::Retry,
            _ => RetryHint::Stop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(status: u16, headers: &[(&str, &str)]) -> HttpReply {
        HttpReply {
            status,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: Box::new(std::io::empty()),
        }
    }

    #[test]
    fn classifies_statuses_and_retry_after() {
        assert_eq!(status_error(&reply(401, &[])), CloudError::AuthExpired);
        assert_eq!(status_error(&reply(412, &[])), CloudError::RevMismatch);
        assert_eq!(status_error(&reply(507, &[])), CloudError::QuotaFull);
        assert_eq!(status_error(&reply(502, &[])), CloudError::Offline);
        assert_eq!(
            status_error(&reply(429, &[("Retry-After", "120")])),
            CloudError::RateLimited {
                retry_after: Some(Duration::from_secs(120))
            }
        );
        assert_eq!(
            status_error(&reply(400, &[])),
            CloudError::Other("http status 400".into())
        );
    }

    #[test]
    fn transfer_timeout_grows_with_the_bytes_but_keeps_the_deadline() {
        let background = RequestBudget::BACKGROUND;
        assert_eq!(background.transfer_timeout(1024).unwrap(), REQUEST_TIMEOUT);
        let chunk = background.transfer_timeout(UPLOAD_CHUNK_BYTES).unwrap();
        assert_eq!(chunk, Duration::from_secs(80));
        let pre_play = RequestBudget::until(Instant::now() + Duration::from_secs(15));
        let pre_play_chunk = pre_play.transfer_timeout(UPLOAD_CHUNK_BYTES).unwrap();
        assert!(pre_play_chunk <= PRE_PLAY_REQUEST_TIMEOUT);
        let spent = RequestBudget::until(Instant::now());
        assert_eq!(spent.transfer_timeout(1).unwrap_err(), CloudError::Offline);
    }

    #[test]
    fn offline_switch_is_one_try() {
        let client = http_download_util::build_http_client("Nostyra-test", true);
        let mut calls = 0;
        let result: Result<(), _> = with_retry(&client, RetryPolicy::default(), || {
            calls += 1;
            Err(CloudError::Offline)
        });
        assert_eq!((result, calls), (Err(CloudError::Offline), 0));
        let form = post_form(&client, "https://x.invalid/token", "a=b", REQUEST_TIMEOUT);
        assert_eq!(form.unwrap_err(), CloudError::Offline);
    }
}
