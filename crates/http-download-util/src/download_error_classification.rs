//! Download failures grouped by what the caller should do next: remember a
//! miss (`NotFound`), retry later (`Transient`, `RateLimited`), or give up
//! (`Permanent`, `TooLarge`, `Io`). Callers persist only the final classes, so
//! an offline session never poisons a retry window.

use std::time::Duration;

/// Longest `Retry-After` we honour; a server asking for more is treated as
/// transient for this session instead of stalling a worker for minutes.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(10);

/// Why a download did not produce a body.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    /// HTTP 404: the file does not exist on this server.
    #[error("not found")]
    NotFound,

    /// Network trouble or server overload (offline switch, DNS, connect,
    /// timeout, reset, 429/5xx without `Retry-After`): worth retrying later.
    #[error("temporary download failure: {0}")]
    Transient(String),

    /// HTTP 429/503 with a `Retry-After` the caller should wait out (capped
    /// at 10 s) before the next attempt.
    #[error("server asked to retry after {retry_after:?} (http {status})")]
    RateLimited { status: u16, retry_after: Duration },

    /// A failure retrying cannot fix (bad URL, non-HTTPS, other 4xx, TLS
    /// policy, redirect loop).
    #[error("download failed: {0}")]
    Permanent(String),

    /// Body (announced or streamed) is bigger than the caller's cap.
    #[error("download exceeds the {limit_bytes} byte size limit")]
    TooLarge { limit_bytes: u64 },

    /// The caller's cancel flag was set.
    #[error("download cancelled")]
    Cancelled,

    /// Local I/O failure (or a stream read failing mid-body).
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

impl DownloadError {
    /// Wraps an I/O error with the action that failed, for readable messages.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }

    /// True for failures a later attempt may not hit (retry / do not persist).
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Transient(_) | Self::RateLimited { .. })
    }

    /// Classifies a non-success HTTP status. `retry_after` is the parsed
    /// `Retry-After` header (seconds form) when the server sent one.
    pub fn from_status(status: u16, retry_after: Option<Duration>) -> Self {
        match (status, retry_after) {
            (404, _) => Self::NotFound,
            (429 | 503, Some(wait)) => Self::RateLimited {
                status,
                retry_after: wait.min(MAX_RETRY_AFTER),
            },
            (429 | 500..=599, _) => Self::Transient(format!("http status {status}")),
            _ => Self::Permanent(format!("http status {status}")),
        }
    }
}

impl From<ureq::Error> for DownloadError {
    fn from(error: ureq::Error) -> Self {
        use ureq::Error as E;
        match error {
            E::StatusCode(status) => Self::from_status(status, None),
            E::Timeout(_)
            | E::Io(_)
            | E::HostNotFound
            | E::ConnectionFailed
            | E::BodyStalled
            | E::Protocol(_)
            | E::Tls(_) => Self::Transient(error.to_string()),
            other => Self::Permanent(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_statuses() {
        assert!(matches!(
            DownloadError::from_status(404, None),
            DownloadError::NotFound
        ));
        assert!(DownloadError::from_status(429, None).is_transient());
        assert!(DownloadError::from_status(502, None).is_transient());
        assert!(!DownloadError::from_status(403, None).is_transient());
        assert!(matches!(
            DownloadError::from_status(503, Some(Duration::from_secs(600))),
            DownloadError::RateLimited { retry_after, .. } if retry_after == MAX_RETRY_AFTER
        ));
    }

    #[test]
    fn network_errors_are_transient() {
        assert!(DownloadError::from(ureq::Error::HostNotFound).is_transient());
        assert!(DownloadError::from(ureq::Error::ConnectionFailed).is_transient());
        assert!(!DownloadError::from(ureq::Error::TooManyRedirects).is_transient());
    }
}
