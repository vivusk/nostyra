//! Data-driven OAuth description of one provider: authorize / token URLs, scopes,
//! the loopback redirect rule, extra authorize parameters and the client id (+ the
//! non-confidential desktop secret Google issues). The sign-in flow is generic over
//! this; a provider adds only its `OAuthEndpoints` and an account-label callback.

use std::time::Duration;

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

use crate::secret_string::SecretString;

/// Research default for Dropbox (exact redirect URIs incl. port, registered in the
/// app console); phase 00 confirms. Google and Microsoft accept any loopback port.
pub const DROPBOX_REDIRECT_PORTS: &[u16] = &[53682, 53683, 53684];
/// How long the loopback listener waits for the browser redirect.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// RFC 3986 unreserved characters stay as they are; everything else is encoded.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Host written in the redirect URI. The listener always binds `127.0.0.1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectHost {
    /// `http://127.0.0.1:<port>/` (Google, Dropbox).
    Ipv4Loopback,
    /// `http://localhost:<port>/` (Microsoft only registers `localhost`).
    Localhost,
}

/// Which loopback ports the provider accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortRule {
    /// Any port: bind port 0.
    Any,
    /// Only these registered ports: the first free one is used.
    Fixed(&'static [u16]),
}

/// Loopback redirect rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopbackRedirect {
    pub host: RedirectHost,
    pub ports: PortRule,
}

impl LoopbackRedirect {
    /// The redirect URI for a bound `port`.
    pub fn uri(&self, port: u16) -> String {
        match self.host {
            RedirectHost::Ipv4Loopback => format!("http://127.0.0.1:{port}/"),
            RedirectHost::Localhost => format!("http://localhost:{port}/"),
        }
    }
}

/// Best-effort token revocation at sign-out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenRevoke {
    /// The API has none (Microsoft personal accounts): files are deleted only.
    Unsupported,
    /// RFC 7009 style form POST `token=<refresh token>` (Google).
    FormToken { url: String },
    /// POST with `Authorization: Bearer <access token>` and no body (Dropbox).
    BearerPost { url: String },
}

/// Everything the generic OAuth code needs about one provider.
#[derive(Debug, Clone)]
pub struct OAuthEndpoints {
    pub authorize_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub redirect: LoopbackRedirect,
    /// e.g. `access_type=offline` (Google), `token_access_type=offline` (Dropbox).
    pub extra_auth_params: Vec<(String, String)>,
    pub client_id: String,
    /// Google's desktop "secret" (not confidential for native apps); `None` elsewhere.
    pub client_secret: Option<SecretString>,
    pub revoke: TokenRevoke,
}

impl OAuthEndpoints {
    /// The URL opened in the system browser (code flow + PKCE S256 + `state`).
    pub fn authorization_url(&self, redirect_uri: &str, challenge: &str, state: &str) -> String {
        let scope = self.scopes.join(" ");
        let mut pairs: Vec<(&str, &str)> = vec![
            ("response_type", "code"),
            ("client_id", &self.client_id),
            ("redirect_uri", redirect_uri),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("state", state),
        ];
        if !scope.is_empty() {
            pairs.push(("scope", &scope));
        }
        pairs.extend(
            self.extra_auth_params
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        let separator = if self.authorize_url.contains('?') {
            '&'
        } else {
            '?'
        };
        format!("{}{separator}{}", self.authorize_url, form_encode(&pairs))
    }
}

/// `key=value&…` with every component percent-encoded (query strings and
/// `application/x-www-form-urlencoded` bodies).
pub fn form_encode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", encode_component(key), encode_component(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Percent-encodes one URL component (RFC 3986 unreserved characters kept).
pub fn encode_component(value: &str) -> String {
    utf8_percent_encode(value, COMPONENT).to_string()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Endpoints of a made-up provider (shared by the OAuth tests).
    pub(crate) fn endpoints() -> OAuthEndpoints {
        OAuthEndpoints {
            authorize_url: "https://auth.example/authorize".into(),
            token_url: "https://auth.example/token".into(),
            scopes: vec!["files.read".into(), "offline_access".into()],
            redirect: LoopbackRedirect {
                host: RedirectHost::Localhost,
                ports: PortRule::Any,
            },
            extra_auth_params: vec![("prompt".into(), "consent".into())],
            client_id: "client 1".into(),
            client_secret: Some(SecretString::new("shh")),
            revoke: TokenRevoke::Unsupported,
        }
    }

    #[test]
    fn authorization_url_carries_pkce_state_scopes_and_extras() {
        let redirect = endpoints().redirect.uri(5000);
        assert_eq!(redirect, "http://localhost:5000/");
        let url = endpoints().authorization_url(&redirect, "chal", "st/1");
        assert_eq!(
            url,
            "https://auth.example/authorize?response_type=code&client_id=client%201\
             &redirect_uri=http%3A%2F%2Flocalhost%3A5000%2F&code_challenge=chal\
             &code_challenge_method=S256&state=st%2F1&scope=files.read%20offline_access\
             &prompt=consent"
        );
        assert!(
            !url.contains("shh"),
            "client secret never goes to the browser"
        );
    }

    #[test]
    fn ipv4_redirect_and_debug_redaction() {
        let redirect = LoopbackRedirect {
            host: RedirectHost::Ipv4Loopback,
            ports: PortRule::Fixed(DROPBOX_REDIRECT_PORTS),
        };
        assert_eq!(redirect.uri(53682), "http://127.0.0.1:53682/");
        assert!(!format!("{:?}", endpoints()).contains("shh"));
    }
}
