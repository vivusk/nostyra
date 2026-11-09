//! Token endpoint calls: authorization code (+ PKCE verifier) → tokens, refresh token
//! → tokens. `invalid_grant` (revoked / expired / other-PC token) → `AuthExpired`
//! ("Sign in again"). Error messages carry the OAuth error code only.

use std::time::{Duration, Instant};

use http_download_util::RetryPolicy;
use serde::Deserialize;

use crate::http_transport::{post_form, with_retry, FormReply, HttpTransport, RequestBudget};
use crate::oauth_endpoints::{form_encode, OAuthEndpoints};
use crate::secret_string::SecretString;
use crate::CloudError;

/// An access token is refreshed this long before it expires.
pub const REFRESH_MARGIN: Duration = Duration::from_secs(60);
/// Lifetime assumed when the reply has no `expires_in`.
const DEFAULT_EXPIRES_IN_SECS: u64 = 3600;
/// Longest lifetime believed (a negative / huge / fractional `expires_in` = this):
/// `Instant + Duration` must never overflow.
const MAX_EXPIRES_IN_SECS: u64 = 86_400;

/// Short-lived bearer token (memory only, never stored).
#[derive(Debug, Clone)]
pub struct AccessToken {
    pub value: SecretString,
    pub expires_at: Instant,
}

impl AccessToken {
    /// Still usable for at least [`REFRESH_MARGIN`].
    pub fn is_fresh(&self, now: Instant) -> bool {
        now + REFRESH_MARGIN < self.expires_at
    }

    /// `Authorization` header value.
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.value.expose())
    }
}

/// What the token endpoint granted. `refresh_token` is `None` when the provider
/// kept the old one (Google / Dropbox refresh); Microsoft rotates it every time.
#[derive(Debug, Clone)]
pub struct TokenGrant {
    pub access_token: AccessToken,
    pub refresh_token: Option<SecretString>,
    pub scope: Option<String>,
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: String,
    expires_in: Option<serde_json::Number>,
    refresh_token: Option<String>,
    scope: Option<String>,
}

#[derive(Deserialize)]
struct ErrorReply {
    error: String,
}

/// Exchanges the browser's authorization code. Not retried after an ambiguous
/// failure: a code is single-use.
pub fn exchange_code(
    transport: &dyn HttpTransport,
    endpoints: &OAuthEndpoints,
    code: &SecretString,
    verifier: &SecretString,
    redirect_uri: &str,
) -> Result<TokenGrant, CloudError> {
    let mut pairs = vec![
        ("grant_type", "authorization_code"),
        ("code", code.expose()),
        ("redirect_uri", redirect_uri),
        ("client_id", endpoints.client_id.as_str()),
        ("code_verifier", verifier.expose()),
    ];
    let budget = RequestBudget::BACKGROUND;
    let policy = RetryPolicy {
        idempotent: false,
        ..budget.retry_policy()
    };
    request_tokens(transport, endpoints, &mut pairs, budget, policy)
}

/// Gets a new access token (and, on Microsoft, a rotated refresh token) within
/// `budget` (pre-play: short timeout, no retry past the deadline).
pub fn refresh_access_token(
    transport: &dyn HttpTransport,
    endpoints: &OAuthEndpoints,
    refresh_token: &SecretString,
    budget: RequestBudget,
) -> Result<TokenGrant, CloudError> {
    let mut pairs = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token.expose()),
        ("client_id", endpoints.client_id.as_str()),
    ];
    request_tokens(
        transport,
        endpoints,
        &mut pairs,
        budget,
        budget.retry_policy(),
    )
}

fn request_tokens<'a>(
    transport: &dyn HttpTransport,
    endpoints: &'a OAuthEndpoints,
    pairs: &mut Vec<(&'a str, &'a str)>,
    budget: RequestBudget,
    policy: RetryPolicy,
) -> Result<TokenGrant, CloudError> {
    if let Some(secret) = &endpoints.client_secret {
        pairs.push(("client_secret", secret.expose()));
    }
    let form = form_encode(pairs);
    let FormReply { status, body, .. } = with_retry(transport, policy, || {
        let timeout = budget.request_timeout()?;
        let reply = post_form(transport, &endpoints.token_url, &form, timeout)?;
        match reply.status {
            429 | 503 => Err(CloudError::RateLimited {
                retry_after: reply.retry_after,
            }),
            500..=599 => Err(CloudError::Offline),
            _ => Ok(reply),
        }
    })?;
    if !(200..300).contains(&status) {
        return Err(token_error(status, &body));
    }
    let reply: TokenReply = serde_json::from_slice(&body)
        .map_err(|_| CloudError::Other("unreadable token reply".into()))?;
    let expires_in = token_lifetime_secs(reply.expires_in.as_ref());
    Ok(TokenGrant {
        access_token: AccessToken {
            value: SecretString::new(reply.access_token),
            expires_at: Instant::now() + Duration::from_secs(expires_in),
        },
        refresh_token: reply
            .refresh_token
            .filter(|token| !token.is_empty())
            .map(SecretString::new),
        scope: reply.scope,
    })
}

/// The access token lifetime from `expires_in` (missing = the default), clamped to
/// [`MAX_EXPIRES_IN_SECS`].
pub(crate) fn token_lifetime_secs(expires_in: Option<&serde_json::Number>) -> u64 {
    expires_in.map_or(DEFAULT_EXPIRES_IN_SECS, |n| {
        n.as_u64()
            .map_or(MAX_EXPIRES_IN_SECS, |secs| secs.min(MAX_EXPIRES_IN_SECS))
    })
}

/// 4xx token reply → `AuthExpired` for `invalid_grant`, else the bare error code.
fn token_error(status: u16, body: &[u8]) -> CloudError {
    match serde_json::from_slice::<ErrorReply>(body) {
        Ok(reply) if reply.error == "invalid_grant" => CloudError::AuthExpired,
        Ok(reply) => {
            let code: String = reply
                .error
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .take(64)
                .collect();
            CloudError::Other(format!("token endpoint refused: {code}"))
        }
        Err(_) => CloudError::Other(format!("token endpoint http status {status}")),
    }
}

#[cfg(test)]
#[path = "oauth_token_client_tests.rs"]
mod tests;
