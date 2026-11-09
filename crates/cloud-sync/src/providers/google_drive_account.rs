//! Google's OAuth description (desktop client: any `127.0.0.1` port, the
//! non-confidential secret sent on exchange + refresh, `access_type=offline` +
//! `prompt=consent` so every sign-in yields a refresh token) and the account
//! callback of the sign-in (`about?fields=user`).

use serde::Deserialize;

use super::google_drive_errors::drive_error;
use super::google_drive_requests::DRIVE_API;
use crate::cloud_app_client_ids::ClientCredentials;
use crate::http_transport::{
    read_body, HttpTransport, CONNECT_TIMEOUT, MAX_JSON_REPLY_BYTES, REQUEST_TIMEOUT,
};
use crate::oauth_endpoints::{
    LoopbackRedirect, OAuthEndpoints, PortRule, RedirectHost, TokenRevoke,
};
use crate::oauth_sign_in::AccountIdentity;
use crate::oauth_token_client::AccessToken;
use crate::CloudError;

/// Only files this app created or opened (non-sensitive scope, Q9).
pub const GOOGLE_DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

/// The OAuth description of Google (desktop client: any loopback port, secret sent).
pub fn google_drive_endpoints(credentials: ClientCredentials) -> OAuthEndpoints {
    OAuthEndpoints {
        authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
        token_url: "https://oauth2.googleapis.com/token".into(),
        scopes: vec![GOOGLE_DRIVE_SCOPE.into()],
        redirect: LoopbackRedirect {
            host: RedirectHost::Ipv4Loopback,
            ports: PortRule::Any,
        },
        extra_auth_params: vec![
            ("access_type".into(), "offline".into()),
            ("prompt".into(), "consent".into()),
        ],
        client_id: credentials.client_id,
        client_secret: credentials.client_secret,
        revoke: TokenRevoke::FormToken {
            url: "https://oauth2.googleapis.com/revoke".into(),
        },
    }
}

#[derive(Deserialize)]
struct About {
    user: AboutUser,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AboutUser {
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    email_address: String,
    #[serde(default)]
    permission_id: String,
}

/// Who signed in (`about?fields=user`): the sign-in callback.
pub fn fetch_google_account(
    transport: &dyn HttpTransport,
    access: &AccessToken,
) -> Result<AccountIdentity, CloudError> {
    let url = format!("{DRIVE_API}/about?fields=user(displayName%2CemailAddress%2CpermissionId)");
    let bearer = access.bearer();
    let reply = transport.send(http_download_util::HttpRequest {
        method: http_download_util::HttpMethod::Get,
        url: &url,
        headers: &[("Authorization", &bearer)],
        body: http_download_util::RequestBody::Empty,
        connect_timeout: Some(CONNECT_TIMEOUT),
        timeout: Some(REQUEST_TIMEOUT),
    })?;
    if !(200..300).contains(&reply.status) {
        return Err(drive_error(reply));
    }
    let body = read_body(reply, MAX_JSON_REPLY_BYTES)?;
    let user = serde_json::from_slice::<About>(&body)
        .map_err(|_| CloudError::Other("unreadable drive account reply".into()))?
        .user;
    let label = [&user.email_address, &user.display_name]
        .into_iter()
        .find(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| "Google Drive".into());
    let account_id = [user.permission_id, user.email_address]
        .into_iter()
        .find(|value| !value.is_empty())
        .ok_or_else(|| CloudError::Other("drive account without an id".into()))?;
    Ok(AccountIdentity {
        account_id,
        account_label: label,
    })
}
