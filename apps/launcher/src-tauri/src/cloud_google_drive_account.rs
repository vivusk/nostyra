//! Google Drive in the launcher: the signed-in account from `<data>/cloud/account.json`
//! and the DPAPI `refresh-token.bin` → a `GoogleDriveProvider`; the browser sign-in
//! (loopback + PKCE, `about` for the label) and storing its result. Available only
//! when a client id is configured (`NOSTYRA_GOOGLE_CLIENT_ID` until phase 09 commits
//! the production one).

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use cloud_sync::providers::google_drive_account::{fetch_google_account, google_drive_endpoints};
use cloud_sync::providers::google_drive_provider::GoogleDriveProvider;
use cloud_sync::{
    account_key, load_account, load_refresh_token, save_signed_in_account, sign_in,
    store_refresh_token, ClientCredentials, HttpTransport, OAuthProvider, OAuthSession,
    SignInError, SignedInAccount, StoredAccount,
};
use config_layers::{global_config_path, set_layer_value, CloudProviderId, Value};

use crate::cloud_provider_factory::{AccountState, CloudAccount};

/// Shown while Google Drive is picked but nobody signed in on this PC.
pub const GOOGLE_SIGN_IN_NEEDED: &str = "Sign in to Google Drive.";
/// The stored sign-in cannot be used any more (token file gone / undecryptable).
pub const SIGN_IN_AGAIN: &str = "Sign in again.";

/// The shared HTTPS client for cloud calls (same User-Agent / offline switch as
/// `AppState::http`).
pub fn cloud_http_transport() -> Arc<dyn HttpTransport> {
    Arc::new(http_download_util::build_http_client(
        concat!("Nostyra/", env!("CARGO_PKG_VERSION")),
        std::env::var("NOSTYRA_OFFLINE").is_ok_and(|v| v == "1"),
    ))
}

/// The stored Google account's id (cache key), or why there is none.
pub fn google_account_id(data_root: &Path) -> Result<String, AccountState> {
    match load_account(data_root) {
        Ok(Some(stored)) if stored.provider == OAuthProvider::GoogleDrive => Ok(stored.account_id),
        Ok(_) => Err(AccountState::NeedsSignIn(GOOGLE_SIGN_IN_NEEDED)),
        Err(err) => {
            tracing::warn!(target: "cloud_sync", %err, "unreadable cloud account file");
            Err(AccountState::NeedsSignIn(SIGN_IN_AGAIN))
        }
    }
}

/// The Google Drive account over `transport` (`Ready` or why it needs a sign-in).
pub fn google_drive_account(
    data_root: &Path,
    credentials: ClientCredentials,
    transport: Arc<dyn HttpTransport>,
) -> AccountState {
    let stored = match load_account(data_root) {
        Ok(Some(stored)) if stored.provider == OAuthProvider::GoogleDrive => stored,
        _ => return AccountState::NeedsSignIn(GOOGLE_SIGN_IN_NEEDED),
    };
    let Ok(refresh_token) = load_refresh_token(data_root) else {
        return AccountState::NeedsSignIn(SIGN_IN_AGAIN);
    };
    let token_root = data_root.to_path_buf();
    let session = OAuthSession::new(
        google_drive_endpoints(credentials),
        transport,
        refresh_token,
        None,
        Box::new(move |rotated| store_refresh_token(&token_root, rotated)),
    );
    let label = stored.account_label.clone();
    AccountState::Ready(CloudAccount {
        provider: Arc::new(GoogleDriveProvider::new(session, label.clone())),
        account_key: account_key(CloudProviderId::GoogleDrive.as_str(), &stored.account_id),
        label,
    })
}

/// Runs the browser sign-in (`open_browser` gets the authorize URL; `cancel` aborts).
pub fn google_sign_in(
    transport: &dyn HttpTransport,
    credentials: ClientCredentials,
    open_browser: impl Fn(&str),
    cancel: &AtomicBool,
) -> Result<SignedInAccount, SignInError> {
    let endpoints = google_drive_endpoints(credentials);
    sign_in(
        transport,
        &endpoints,
        open_browser,
        |access| fetch_google_account(transport, access),
        cancel,
    )
}

/// Stores a finished sign-in (token, then `account.json`) and selects Google Drive
/// in the global config.
pub fn store_google_sign_in(data_root: &Path, signed: &SignedInAccount) -> std::io::Result<()> {
    let account = StoredAccount {
        provider: OAuthProvider::GoogleDrive,
        account_id: signed.account.account_id.clone(),
        account_label: signed.account.account_label.clone(),
        scopes: signed.scopes.clone(),
        signed_in_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs()),
    };
    save_signed_in_account(data_root, &account, &signed.refresh_token)?;
    let provider = Value::from(CloudProviderId::GoogleDrive.as_str());
    set_layer_value(
        &global_config_path(data_root),
        "cloud.provider",
        Some(provider),
    )
    .map_err(std::io::Error::other)
}

#[cfg(test)]
#[path = "cloud_google_drive_account_tests.rs"]
mod tests;
