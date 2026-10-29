//! RetroAchievements login kept between sessions: `<data>/achievements/credentials.json`
//! with the username and API token only (never the password). Plaintext by user
//! decision (Q2; the UI says so); owner-only permissions on Unix, the user profile's
//! default ACL on Windows. The launcher hands the same pair to the runner via env.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::offline_response_cache::OfflineResponseCache;

/// Env var with the username (launcher → runner).
pub const USER_ENV: &str = "NOSTYRA_RA_USER";
/// Env var with the API token (launcher → runner; env, not argv: argv is visible in
/// process lists).
pub const TOKEN_ENV: &str = "NOSTYRA_RA_TOKEN";

/// Username + API token.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub user: String,
    pub token: String,
}

impl std::fmt::Debug for Credentials {
    /// The token never reaches logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("user", &self.user)
            .field("token", &"<redacted>")
            .finish()
    }
}

impl Credentials {
    fn is_complete(&self) -> bool {
        !self.user.trim().is_empty() && !self.token.trim().is_empty()
    }
}

/// `<data>/achievements/credentials.json`.
pub fn credentials_path(data_dir: &Path) -> PathBuf {
    data_dir.join("achievements").join("credentials.json")
}

/// Stored login; `Ok(None)` when logged out (no file) or the file is incomplete.
pub fn load_credentials(data_dir: &Path) -> std::io::Result<Option<Credentials>> {
    match std::fs::read(credentials_path(data_dir)) {
        Ok(bytes) => {
            let credentials: Credentials =
                serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
            Ok(credentials.is_complete().then_some(credentials))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Writes the login atomically (owner-only on Unix). A login of another account (or
/// over an unreadable file) drops the offline answer cache: it holds the old token.
pub fn save_credentials(data_dir: &Path, credentials: &Credentials) -> std::io::Result<()> {
    let same_user = matches!(load_credentials(data_dir),
        Ok(Some(previous)) if previous.user.eq_ignore_ascii_case(&credentials.user));
    if !same_user {
        OfflineResponseCache::clear(data_dir)?;
    }
    let path = credentials_path(data_dir);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(credentials).map_err(std::io::Error::other)?;
    config_layers::write_atomically(&path, &json)?;
    restrict_to_owner(&path)
}

/// Owner-only permissions on Unix (credentials, cached login answer); the user
/// profile's default ACL on Windows.
pub(crate) fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Logs out: removes the file and the offline answer cache (already absent is fine).
pub fn delete_credentials(data_dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(credentials_path(data_dir)) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
        _ => OfflineResponseCache::clear(data_dir),
    }
}

/// Login passed by the launcher (`NOSTYRA_RA_USER` + `NOSTYRA_RA_TOKEN`), if both set.
pub fn credentials_from_env() -> Option<Credentials> {
    let credentials = Credentials {
        user: std::env::var(USER_ENV).ok()?,
        token: std::env::var(TOKEN_ENV).ok()?,
    };
    credentials.is_complete().then_some(credentials)
}

/// Runner rule: env from the launcher first, else the credentials file (CLI use).
pub fn resolve_credentials(data_dir: &Path) -> Option<Credentials> {
    credentials_from_env().or_else(|| match load_credentials(data_dir) {
        Ok(credentials) => credentials,
        Err(err) => {
            tracing::warn!(%err, "unreadable achievements credentials file");
            None
        }
    })
}

#[cfg(test)]
#[path = "credentials_file_tests.rs"]
mod tests;
