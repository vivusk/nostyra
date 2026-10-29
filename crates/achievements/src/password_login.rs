//! Launcher login: username + password → `r=login2` built by `rc_api` (no rc_client)
//! → API token. The password is used for this one request only: held in [`Password`]
//! (overwritten on drop), wiped from every request copy, never logged or written.

use crate::ffi::{login_request, parse_login_response};
use crate::server_transport::{is_retryable_response, ServerTransport};
use crate::{RC_ACCESS_DENIED, RC_EXPIRED_TOKEN, RC_INVALID_CREDENTIALS};

/// A password that is overwritten in memory when dropped (best effort, no dependency).
pub struct Password(String);

impl Password {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}

impl Drop for Password {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Overwrites a string's bytes before its buffer is freed.
fn wipe(text: &mut String) {
    let mut bytes = std::mem::take(text).into_bytes();
    bytes.iter_mut().for_each(|byte| *byte = 0);
    std::hint::black_box(&bytes);
}

/// A successful login: what the credentials file and the Settings line need.
#[derive(Clone, PartialEq, Eq)]
pub struct LoginSuccess {
    pub username: String,
    pub display_name: String,
    pub token: String,
    pub score: u32,
    pub score_softcore: u32,
}

impl std::fmt::Debug for LoginSuccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginSuccess")
            .field("username", &self.username)
            .field("score", &self.score)
            .finish_non_exhaustive()
    }
}

/// Why a login failed, grouped for the UI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoginError {
    #[error("wrong username or password")]
    InvalidCredentials,
    #[error("the login has expired; log in again")]
    ExpiredToken,
    #[error("access denied: {0}")]
    AccessDenied(String),
    #[error("cannot reach RetroAchievements: {0}")]
    Network(String),
    #[error("RetroAchievements refused the login: {0}")]
    Server(String),
}

/// Logs in with a password and returns the token. Blocking (one HTTP request).
pub fn login_with_password(
    transport: &dyn ServerTransport,
    username: &str,
    password: &Password,
) -> Result<LoginSuccess, LoginError> {
    let mut request = login_request(username, Some(password.expose()), None)
        .map_err(|rc| LoginError::Server(crate::error_text(rc)))?;
    let response = transport.call(&request);
    wipe(&mut request.post_data);
    if is_retryable_response(&response) {
        return Err(LoginError::Network(response.body));
    }
    match parse_login_response(&response) {
        Ok(reply) => Ok(LoginSuccess {
            username: reply.username,
            display_name: reply.display_name,
            token: reply.token,
            score: reply.score,
            score_softcore: reply.score_softcore,
        }),
        Err(failure) => Err(match failure.code {
            RC_INVALID_CREDENTIALS => LoginError::InvalidCredentials,
            RC_EXPIRED_TOKEN => LoginError::ExpiredToken,
            RC_ACCESS_DENIED => LoginError::AccessDenied(failure.message),
            _ => LoginError::Server(failure.message),
        }),
    }
}

#[cfg(test)]
#[path = "password_login_tests.rs"]
mod tests;
