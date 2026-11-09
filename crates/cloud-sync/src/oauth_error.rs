//! Why a sign-in did not produce an account. Messages are safe to show and log
//! (no codes, tokens or URLs).

use crate::CloudError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignInError {
    /// The user cancelled in Nostyra or denied consent in the browser.
    #[error("sign-in cancelled")]
    Cancelled,
    /// No browser redirect within the sign-in timeout.
    #[error("sign-in timed out")]
    TimedOut,
    /// The redirect carried another sign-in's `state` (stale tab or forged request).
    #[error("sign-in answer did not match this request")]
    StateMismatch,
    /// Every registered redirect port is in use by another program.
    #[error("no free sign-in port (close the program using port {0})")]
    PortsBusy(String),
    /// The provider answered with an OAuth error (`error=` code only).
    #[error("sign-in refused by the provider: {0}")]
    Refused(String),
    /// Token endpoint / account lookup failed.
    #[error(transparent)]
    Cloud(#[from] CloudError),
    /// Local failure (socket, RNG, token file).
    #[error("sign-in failed: {0}")]
    Local(String),
}

impl From<std::io::Error> for SignInError {
    fn from(err: std::io::Error) -> Self {
        Self::Local(err.to_string())
    }
}
