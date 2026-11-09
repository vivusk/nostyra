//! Public OAuth client ids of the Nostyra apps (native apps cannot keep secrets, so
//! they are committed like rclone's). Empty until the apps are registered (phase 00 /
//! M0): such a provider is unavailable unless overridden. Lookup order per value:
//! runtime env `NOSTYRA_<PROVIDER>_CLIENT_ID` (forks / dev) → the same name at build
//! time (`option_env!`) → the committed constant.

use serde::{Deserialize, Serialize};

use crate::secret_string::SecretString;

const GOOGLE_CLIENT_ID: &str = "";
/// Google's "client secret" of a Desktop-app client. Not confidential: Google's
/// native-app OAuth docs state installed apps cannot keep it secret (PKCE protects
/// the flow). Committed with the id once registered.
const GOOGLE_CLIENT_SECRET: &str = "";
const MICROSOFT_CLIENT_ID: &str = "";
const DROPBOX_CLIENT_ID: &str = "";

/// The OAuth providers (config / `account.json` spelling via serde).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OAuthProvider {
    #[serde(rename = "google_drive")]
    GoogleDrive,
    #[serde(rename = "onedrive")]
    OneDrive,
    #[serde(rename = "dropbox")]
    Dropbox,
}

/// Client id (+ Google's non-confidential secret).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCredentials {
    pub client_id: String,
    pub client_secret: Option<SecretString>,
}

/// Credentials for `provider` from env / build env / committed ids; `None` when no
/// id is configured (provider unavailable in this build).
pub fn client_credentials(provider: OAuthProvider) -> Option<ClientCredentials> {
    client_credentials_with(provider, |name| std::env::var(name).ok())
}

/// [`client_credentials`] with an injected runtime env (tests).
pub fn client_credentials_with(
    provider: OAuthProvider,
    runtime_env: impl Fn(&str) -> Option<String>,
) -> Option<ClientCredentials> {
    let pick = |name: &str, build: Option<&'static str>, committed: &'static str| {
        runtime_env(name)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| build.map(str::to_string).filter(|value| !value.is_empty()))
            .or_else(|| (!committed.is_empty()).then(|| committed.to_string()))
    };
    let (client_id, client_secret) = match provider {
        OAuthProvider::GoogleDrive => (
            pick(
                "NOSTYRA_GOOGLE_CLIENT_ID",
                option_env!("NOSTYRA_GOOGLE_CLIENT_ID"),
                GOOGLE_CLIENT_ID,
            ),
            pick(
                "NOSTYRA_GOOGLE_CLIENT_SECRET",
                option_env!("NOSTYRA_GOOGLE_CLIENT_SECRET"),
                GOOGLE_CLIENT_SECRET,
            ),
        ),
        OAuthProvider::OneDrive => (
            pick(
                "NOSTYRA_MICROSOFT_CLIENT_ID",
                option_env!("NOSTYRA_MICROSOFT_CLIENT_ID"),
                MICROSOFT_CLIENT_ID,
            ),
            None,
        ),
        OAuthProvider::Dropbox => (
            pick(
                "NOSTYRA_DROPBOX_CLIENT_ID",
                option_env!("NOSTYRA_DROPBOX_CLIENT_ID"),
                DROPBOX_CLIENT_ID,
            ),
            None,
        ),
    };
    Some(ClientCredentials {
        client_id: client_id?,
        client_secret: client_secret.map(SecretString::new),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_env_overrides_and_empty_means_unavailable() {
        let env = |name: &str| match name {
            "NOSTYRA_GOOGLE_CLIENT_ID" => Some(" id.apps.googleusercontent.com ".to_string()),
            "NOSTYRA_GOOGLE_CLIENT_SECRET" => Some("GOCSPX-x".to_string()),
            "NOSTYRA_DROPBOX_CLIENT_ID" => Some("  ".to_string()),
            _ => None,
        };
        let google = client_credentials_with(OAuthProvider::GoogleDrive, env).unwrap();
        assert_eq!(google.client_id, "id.apps.googleusercontent.com");
        assert_eq!(google.client_secret.unwrap().expose(), "GOCSPX-x");
        if option_env!("NOSTYRA_DROPBOX_CLIENT_ID").is_none() && DROPBOX_CLIENT_ID.is_empty() {
            assert_eq!(client_credentials_with(OAuthProvider::Dropbox, env), None);
        }
    }

    #[test]
    fn provider_names_match_config_spelling() {
        assert_eq!(
            serde_json::to_string(&OAuthProvider::OneDrive).unwrap(),
            "\"onedrive\""
        );
        assert_eq!(
            serde_json::from_str::<OAuthProvider>("\"google_drive\"").unwrap(),
            OAuthProvider::GoogleDrive
        );
    }
}
