//! Errors of a cloud storage provider (network / account side). Local file errors
//! are [`crate::CloudSyncError`]; the executor wraps both in `SyncUnitError`.

use std::time::Duration;

/// What a provider call can fail with.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CloudError {
    /// No connection / DNS / timeout: play on with local saves, upload later.
    #[error("cloud unreachable")]
    Offline,
    /// The sign-in is no longer valid ("Sign in again").
    #[error("cloud sign-in expired")]
    AuthExpired,
    /// A conditional write lost: the file changed since it was read.
    #[error("remote file changed since it was read")]
    RevMismatch,
    /// The remote file or folder does not exist.
    #[error("remote file not found")]
    NotFound,
    /// The provider throttles us; retry after the hinted delay.
    #[error("cloud rate limit")]
    RateLimited { retry_after: Option<Duration> },
    /// The account's storage is full.
    #[error("cloud storage full")]
    QuotaFull,
    /// A remote file written by a newer Nostyra (never overwritten by this build).
    #[error("cloud data written by a newer Nostyra")]
    UnsupportedRemoteFormat,
    /// Anything else (message safe to log: no tokens, no URLs with codes).
    #[error("cloud error: {0}")]
    Other(String),
}

impl CloudError {
    /// Whether a later retry of the same call may succeed without user action.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Offline | Self::RateLimited { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_errors() {
        assert!(CloudError::Offline.is_transient());
        assert!(CloudError::RateLimited { retry_after: None }.is_transient());
        for err in [
            CloudError::AuthExpired,
            CloudError::RevMismatch,
            CloudError::NotFound,
            CloudError::QuotaFull,
            CloudError::UnsupportedRemoteFormat,
            CloudError::Other("x".into()),
        ] {
            assert!(!err.is_transient(), "{err}");
        }
    }
}
