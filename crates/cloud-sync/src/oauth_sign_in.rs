//! Browser sign-in (authorization code + PKCE through a loopback redirect) and
//! sign-out, generic over [`OAuthEndpoints`]. A provider adds only its endpoints
//! and an account callback (Google `about?fields=user`, Graph `/me`, Dropbox
//! `users/get_current_account`).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use http_download_util::{HttpMethod, HttpRequest, RequestBody};

use crate::http_transport::{
    post_form, HttpTransport, RequestBudget, CONNECT_TIMEOUT, PRE_PLAY_REQUEST_TIMEOUT,
};
use crate::oauth_endpoints::{form_encode, OAuthEndpoints, TokenRevoke, SIGN_IN_TIMEOUT};
use crate::oauth_error::SignInError;
use crate::oauth_loopback_listener::LoopbackListener;
use crate::oauth_pkce::{random_state, PkcePair};
use crate::oauth_token_client::{exchange_code, refresh_access_token, AccessToken};
use crate::secret_string::SecretString;
use crate::token_store_files::{delete_account_files, load_refresh_token};
use crate::CloudError;

/// Who signed in, as the provider's account API reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountIdentity {
    /// Stable provider account id (base-manifest account key).
    pub account_id: String,
    /// Shown in Settings (e-mail or display name).
    pub account_label: String,
}

/// Result of a successful sign-in (the caller stores it with `token_store_files`).
#[derive(Debug, Clone)]
pub struct SignedInAccount {
    pub refresh_token: SecretString,
    pub access_token: AccessToken,
    pub account: AccountIdentity,
    pub scopes: Vec<String>,
}

/// Runs the whole sign-in: opens `open_browser(authorize_url)`, waits ≤ 5 min for
/// the redirect (`cancel` aborts), exchanges the code, asks `fetch_account`.
pub fn sign_in(
    transport: &dyn HttpTransport,
    endpoints: &OAuthEndpoints,
    open_browser: impl Fn(&str),
    fetch_account: impl Fn(&AccessToken) -> Result<AccountIdentity, CloudError>,
    cancel: &AtomicBool,
) -> Result<SignedInAccount, SignInError> {
    sign_in_within(
        transport,
        endpoints,
        open_browser,
        fetch_account,
        cancel,
        SIGN_IN_TIMEOUT,
    )
}

/// [`sign_in`] with an explicit redirect timeout (tests).
pub fn sign_in_within(
    transport: &dyn HttpTransport,
    endpoints: &OAuthEndpoints,
    open_browser: impl Fn(&str),
    fetch_account: impl Fn(&AccessToken) -> Result<AccountIdentity, CloudError>,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<SignedInAccount, SignInError> {
    let pkce = PkcePair::generate()?;
    let state = random_state()?;
    let listener = LoopbackListener::bind(&endpoints.redirect)?;
    let redirect_uri = listener.redirect_uri().to_string();
    open_browser(&endpoints.authorization_url(&redirect_uri, &pkce.challenge, &state));
    let code = listener.wait_for_code(&state, timeout, cancel)?;
    drop(listener);
    if cancel.load(Ordering::Relaxed) {
        return Err(SignInError::Cancelled);
    }
    let grant = exchange_code(transport, endpoints, &code, &pkce.verifier, &redirect_uri)?;
    let refresh_token = grant
        .refresh_token
        .ok_or_else(|| SignInError::Refused("no offline access granted".into()))?;
    let account = fetch_account(&grant.access_token)?;
    let scopes = match grant.scope {
        Some(scope) => scope.split_whitespace().map(str::to_string).collect(),
        None => endpoints.scopes.clone(),
    };
    tracing::info!(target: "cloud_sync", "signed in");
    Ok(SignedInAccount {
        refresh_token,
        access_token: grant.access_token,
        account,
        scopes,
    })
}

/// Signs out: best-effort revoke of the stored token, then deletes `account.json`
/// and `refresh-token.bin`. Cloud data and local saves stay.
pub fn sign_out(
    data_root: &Path,
    transport: &dyn HttpTransport,
    endpoints: &OAuthEndpoints,
) -> std::io::Result<()> {
    if let Ok(refresh_token) = load_refresh_token(data_root) {
        if let Err(err) = revoke(transport, endpoints, &refresh_token) {
            tracing::info!(target: "cloud_sync", %err, "token revoke skipped");
        }
    }
    delete_account_files(data_root)
}

/// Revokes `refresh_token` where the provider has an API for it (best effort, short
/// 10 s bound: the Dropbox-style refresh + revoke share one short budget).
pub fn revoke(
    transport: &dyn HttpTransport,
    endpoints: &OAuthEndpoints,
    refresh_token: &SecretString,
) -> Result<(), CloudError> {
    let status = match &endpoints.revoke {
        TokenRevoke::Unsupported => return Ok(()),
        TokenRevoke::FormToken { url } => {
            let form = form_encode(&[("token", refresh_token.expose())]);
            post_form(transport, url, &form, PRE_PLAY_REQUEST_TIMEOUT)?.status
        }
        TokenRevoke::BearerPost { url } => {
            let budget = RequestBudget::until(Instant::now() + PRE_PLAY_REQUEST_TIMEOUT);
            let access = refresh_access_token(transport, endpoints, refresh_token, budget)?;
            let bearer = access.access_token.bearer();
            let reply = transport.send(HttpRequest {
                method: HttpMethod::Post,
                url,
                headers: &[("Authorization", &bearer)],
                body: RequestBody::Empty,
                connect_timeout: Some(CONNECT_TIMEOUT),
                timeout: Some(budget.request_timeout()?),
            })?;
            reply.status
        }
    };
    match status {
        200..=299 => Ok(()),
        status => Err(CloudError::Other(format!("revoke http status {status}"))),
    }
}

#[cfg(test)]
#[path = "oauth_sign_in_tests.rs"]
mod tests;
