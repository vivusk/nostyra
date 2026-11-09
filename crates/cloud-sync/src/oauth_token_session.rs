//! A signed-in provider's tokens at run time: the refresh token (from the encrypted
//! file) and an in-memory access token, refreshed 60 s before expiry or once after a
//! 401. A rotated refresh token (Microsoft, every refresh) is handed to the
//! `on_rotated` sink at once (the launcher writes `refresh-token.bin` atomically).

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use http_download_util::HttpReply;

use crate::http_transport::{HttpTransport, RequestBudget};
use crate::oauth_endpoints::OAuthEndpoints;
use crate::oauth_token_client::{refresh_access_token, AccessToken};
use crate::secret_string::SecretString;
use crate::CloudError;

/// Persists a rotated refresh token.
pub type RotatedTokenSink = Box<dyn Fn(&SecretString) -> std::io::Result<()> + Send + Sync>;

/// Token state shared by one provider's calls.
pub struct OAuthSession {
    endpoints: OAuthEndpoints,
    transport: Arc<dyn HttpTransport>,
    refresh_token: Mutex<SecretString>,
    access: Mutex<Option<AccessToken>>,
    on_rotated: RotatedTokenSink,
}

impl std::fmt::Debug for OAuthSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthSession")
            .field("token_url", &self.endpoints.token_url)
            .finish_non_exhaustive()
    }
}

impl OAuthSession {
    /// `access` = the token from sign-in (saves one refresh), else `None`.
    pub fn new(
        endpoints: OAuthEndpoints,
        transport: Arc<dyn HttpTransport>,
        refresh_token: SecretString,
        access: Option<AccessToken>,
        on_rotated: RotatedTokenSink,
    ) -> Self {
        Self {
            endpoints,
            transport,
            refresh_token: Mutex::new(refresh_token),
            access: Mutex::new(access),
            on_rotated,
        }
    }

    pub fn transport(&self) -> &dyn HttpTransport {
        self.transport.as_ref()
    }

    pub fn endpoints(&self) -> &OAuthEndpoints {
        &self.endpoints
    }

    /// The current refresh token (sign-out revoke).
    pub fn refresh_token(&self) -> SecretString {
        lock(&self.refresh_token).clone()
    }

    /// A token valid for at least the refresh margin (refreshing within `budget`).
    pub fn access_token(&self, budget: RequestBudget) -> Result<AccessToken, CloudError> {
        let mut access = lock(&self.access);
        if let Some(token) = access.as_ref().filter(|t| t.is_fresh(Instant::now())) {
            return Ok(token.clone());
        }
        let refresh_token = self.refresh_token();
        let grant =
            refresh_access_token(self.transport(), &self.endpoints, &refresh_token, budget)?;
        if let Some(rotated) = grant.refresh_token.filter(|new| *new != refresh_token) {
            if let Err(err) = (self.on_rotated)(&rotated) {
                // In memory it still works; the old token stays valid on disk for now.
                tracing::warn!(target: "cloud_sync", %err, "could not store the rotated refresh token");
            }
            *lock(&self.refresh_token) = rotated;
        }
        *access = Some(grant.access_token.clone());
        Ok(grant.access_token)
    }

    /// Drops the access token (next call refreshes).
    pub fn invalidate(&self) {
        *lock(&self.access) = None;
    }

    /// Runs `call(authorization_header)`; a 401 refreshes once and repeats. A second
    /// 401 reply is returned as is (callers map it to `AuthExpired`). Refreshes run
    /// within `budget`.
    pub fn authorized(
        &self,
        budget: RequestBudget,
        mut call: impl FnMut(&str) -> Result<HttpReply, CloudError>,
    ) -> Result<HttpReply, CloudError> {
        let reply = call(&self.access_token(budget)?.bearer())?;
        if reply.status != 401 {
            return Ok(reply);
        }
        self.invalidate();
        call(&self.access_token(budget)?.bearer())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
