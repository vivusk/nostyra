//! The signed-in account on disk: `<data>/cloud/account.json` (provider, account id
//! and label, scopes, sign-in time — no secrets) and `<data>/cloud/refresh-token.bin`
//! (the refresh token, DPAPI-encrypted on Windows; owner-only plaintext elsewhere,
//! Q14). Both written atomically. A missing / undecryptable token → `AuthExpired`
//! ("Sign in again"); saves and cloud data are never touched here.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cloud_app_client_ids::OAuthProvider;
use crate::secret_string::SecretString;
use crate::CloudError;

/// The account Nostyra is signed in to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredAccount {
    pub provider: OAuthProvider,
    pub account_id: String,
    pub account_label: String,
    pub scopes: Vec<String>,
    /// Unix seconds.
    pub signed_in_at: u64,
}

/// `<data>/cloud/account.json`.
pub fn account_file(data_root: &Path) -> PathBuf {
    data_root.join("cloud").join("account.json")
}

/// `<data>/cloud/refresh-token.bin`.
pub fn refresh_token_file(data_root: &Path) -> PathBuf {
    data_root.join("cloud").join("refresh-token.bin")
}

/// Stores a new sign-in: token first, then the account (an account file always has
/// a token next to it).
pub fn save_signed_in_account(
    data_root: &Path,
    account: &StoredAccount,
    refresh_token: &SecretString,
) -> std::io::Result<()> {
    store_refresh_token(data_root, refresh_token)?;
    let json = serde_json::to_vec_pretty(account).map_err(std::io::Error::other)?;
    write_file(&account_file(data_root), &json)
}

/// Replaces the stored refresh token (Microsoft rotation after every refresh).
pub fn store_refresh_token(data_root: &Path, refresh_token: &SecretString) -> std::io::Result<()> {
    let sealed = seal(refresh_token.expose().as_bytes())?;
    write_file(&refresh_token_file(data_root), &sealed)
}

/// The stored account, `None` when signed out.
pub fn load_account(data_root: &Path) -> std::io::Result<Option<StoredAccount>> {
    match std::fs::read(account_file(data_root)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(std::io::Error::other),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// The refresh token; absent, unreadable or undecryptable → `AuthExpired`.
pub fn load_refresh_token(data_root: &Path) -> Result<SecretString, CloudError> {
    let sealed = std::fs::read(refresh_token_file(data_root)).map_err(|err| {
        if err.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(target: "cloud_sync", %err, "unreadable refresh-token file");
        }
        CloudError::AuthExpired
    })?;
    let plain = unseal(&sealed).map_err(|err| {
        tracing::warn!(target: "cloud_sync", %err, "refresh token not decryptable here");
        CloudError::AuthExpired
    })?;
    match String::from_utf8(plain) {
        Ok(token) if !token.is_empty() => Ok(SecretString::new(token)),
        _ => Err(CloudError::AuthExpired),
    }
}

/// Deletes both files (already absent is fine). Cloud data stays where it is.
pub fn delete_account_files(data_root: &Path) -> std::io::Result<()> {
    for path in [refresh_token_file(data_root), account_file(data_root)] {
        match std::fs::remove_file(&path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
    }
    Ok(())
}

fn write_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    config_layers::write_atomically(path, bytes)?;
    restrict_to_owner(path)
}

#[cfg(windows)]
fn seal(plain: &[u8]) -> std::io::Result<Vec<u8>> {
    crate::token_store_dpapi::protect(plain)
}

#[cfg(windows)]
fn unseal(sealed: &[u8]) -> std::io::Result<Vec<u8>> {
    crate::token_store_dpapi::unprotect(sealed)
}

/// Non-Windows (untested, Q14): plaintext, owner-only permissions.
#[cfg(not(windows))]
fn seal(plain: &[u8]) -> std::io::Result<Vec<u8>> {
    Ok(plain.to_vec())
}

#[cfg(not(windows))]
fn unseal(sealed: &[u8]) -> std::io::Result<Vec<u8>> {
    Ok(sealed.to_vec())
}

fn restrict_to_owner(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
#[path = "token_store_files_tests.rs"]
mod tests;
