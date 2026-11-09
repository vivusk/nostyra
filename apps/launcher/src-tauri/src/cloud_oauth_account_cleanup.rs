//! Forgetting a stored OAuth sign-in (`<data>/cloud/account.json` +
//! `refresh-token.bin`) whenever cloud saves stop using it: Sign out (whatever
//! provider is picked) and a switch to the Local folder. The token is revoked first
//! (best effort, short timeout: a failure only logs), then both files are deleted, so
//! no valid Google token stays on disk unused. Cloud data and saves stay.

use std::path::Path;

use cloud_sync::providers::google_drive_account::google_drive_endpoints;
use cloud_sync::{
    client_credentials, delete_account_files, load_account, ClientCredentials, HttpTransport,
    OAuthProvider,
};

use crate::cloud_google_drive_account::cloud_http_transport;

/// Revokes and deletes any stored OAuth account (no account = nothing to do, no
/// network call).
pub fn forget_oauth_account(data_root: &Path) -> std::io::Result<()> {
    forget_oauth_account_with(
        data_root,
        cloud_http_transport().as_ref(),
        client_credentials(OAuthProvider::GoogleDrive),
    )
}

/// [`forget_oauth_account`] over `transport` with the Google credentials (`None`:
/// none configured in this build → delete without revoking).
pub fn forget_oauth_account_with(
    data_root: &Path,
    transport: &dyn HttpTransport,
    google: Option<ClientCredentials>,
) -> std::io::Result<()> {
    // Google is the only OAuth provider built in: revoke unless the stored account
    // says it belongs to another one (unreadable / missing account.json → try).
    let other = matches!(
        load_account(data_root),
        Ok(Some(stored)) if stored.provider != OAuthProvider::GoogleDrive
    );
    match google.filter(|_| !other) {
        Some(credentials) => {
            cloud_sync::sign_out(data_root, transport, &google_drive_endpoints(credentials))
        }
        None => delete_account_files(data_root),
    }
}

#[cfg(test)]
mod tests {
    use cloud_sync::fake_http_transport::FakeHttpTransport;
    use cloud_sync::{
        account_file, refresh_token_file, save_signed_in_account, CloudError, SecretString,
        StoredAccount,
    };

    use super::*;

    fn credentials() -> ClientCredentials {
        ClientCredentials {
            client_id: "cid.apps.googleusercontent.com".into(),
            client_secret: Some(SecretString::new("GOCSPX-test")),
        }
    }

    fn store_google(data_root: &Path) {
        let account = StoredAccount {
            provider: OAuthProvider::GoogleDrive,
            account_id: "perm-1".into(),
            account_label: "me@example.com".into(),
            scopes: vec![],
            signed_in_at: 0,
        };
        save_signed_in_account(data_root, &account, &SecretString::new("1//r")).unwrap();
    }

    #[test]
    fn stored_google_account_is_revoked_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        store_google(dir.path());
        let http = FakeHttpTransport::new();
        http.push_json(200, "{}");
        forget_oauth_account_with(dir.path(), &http, Some(credentials())).unwrap();
        let requests = http.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url, "https://oauth2.googleapis.com/revoke");
        assert!(requests[0].body_text().contains("token=1%2F%2Fr"));
        assert!(!account_file(dir.path()).exists());
        assert!(!refresh_token_file(dir.path()).exists());
    }

    #[test]
    fn failed_revoke_or_missing_credentials_still_delete() {
        let dir = tempfile::tempdir().unwrap();
        store_google(dir.path());
        let http = FakeHttpTransport::new();
        http.push_error(CloudError::Offline);
        forget_oauth_account_with(dir.path(), &http, Some(credentials())).unwrap();
        assert!(!account_file(dir.path()).exists());
        assert!(!refresh_token_file(dir.path()).exists());

        store_google(dir.path());
        forget_oauth_account_with(dir.path(), &http, None).unwrap();
        assert!(!refresh_token_file(dir.path()).exists());
        assert_eq!(http.requests().len(), 1, "no revoke without credentials");
    }

    #[test]
    fn nothing_stored_is_a_no_op_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let http = FakeHttpTransport::new();
        forget_oauth_account_with(dir.path(), &http, Some(credentials())).unwrap();
        assert!(http.requests().is_empty());
    }
}
