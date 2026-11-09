//! Provider id (`[cloud] provider`) → storage implementation, built lazily from the
//! config (+ the stored sign-in) and cached in `CloudSyncState::account` until the
//! config or the signed-in account changes. Local folder and Google Drive (when a
//! client id is configured); OneDrive / Dropbox answer `Unavailable` until their
//! phases (05–06) land.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cloud_sync::providers::local_folder_provider::LocalFolderProvider;
use cloud_sync::{
    account_key, client_credentials, local_folder_account_id, ClientCredentials,
    CloudStorageProvider, OAuthProvider, SyncScope,
};
use config_layers::{
    global_config_path, resolve_layers_with_warnings, CloudConfig, CloudProviderId,
};

use crate::app_state::lock;
use crate::cloud_google_drive_account::{
    cloud_http_transport, google_account_id, google_drive_account,
};
use crate::cloud_sync_state::CloudSyncState;

/// A usable sync account.
#[derive(Clone)]
pub struct CloudAccount {
    pub provider: Arc<dyn CloudStorageProvider>,
    /// `<provider>-<16 hex>`: names `<data>/cloud/base/<account key>/`.
    pub account_key: String,
    /// Shown in Settings (folder path, e-mail…).
    pub label: String,
}

impl std::fmt::Debug for CloudAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudAccount")
            .field("account_key", &self.account_key)
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

/// The account for the current config and why there is none.
#[derive(Debug, Clone)]
pub enum AccountState {
    /// Cloud saves off.
    Off,
    Ready(CloudAccount),
    /// Configured but not usable: sign in again / choose a reachable folder.
    NeedsSignIn(&'static str),
    /// The provider is not built into this version (no client id / phases 05–06).
    Unavailable(CloudProviderId),
}

/// Cache entry: the config it was built from + the account.
#[derive(Debug, Clone)]
pub struct CachedAccount {
    key: (CloudProviderId, String),
    account: CloudAccount,
}

/// The global `[cloud]` section.
pub fn cloud_config(data_root: &Path) -> CloudConfig {
    let global = global_config_path(data_root);
    resolve_layers_with_warnings(&[global.as_path()], None)
        .0
        .cloud
}

/// The scope the user's toggles give (per-file cap built in).
pub fn sync_scope(config: &CloudConfig) -> SyncScope {
    SyncScope {
        sync_states: config.sync_states,
        ..SyncScope::default()
    }
}

/// The account for `config`, reusing the cached provider while the config and the
/// signed-in account did not change. A Local folder counts as signed in when it is
/// configured and reachable; Google Drive when `account.json` + the token file exist.
pub fn current_account(
    data_root: &Path,
    cloud: &CloudSyncState,
    config: &CloudConfig,
) -> AccountState {
    current_account_with(data_root, cloud, config, || {
        client_credentials(OAuthProvider::GoogleDrive)
    })
}

/// [`current_account`] with injected Google credentials (tests).
pub fn current_account_with(
    data_root: &Path,
    cloud: &CloudSyncState,
    config: &CloudConfig,
    google: impl FnOnce() -> Option<ClientCredentials>,
) -> AccountState {
    let google = match config.provider {
        CloudProviderId::GoogleDrive => match google() {
            Some(credentials) => Some(credentials),
            None => return forget(cloud, AccountState::Unavailable(config.provider)),
        },
        _ => None,
    };
    let discriminator = match account_discriminator(data_root, config) {
        Ok(discriminator) => discriminator,
        Err(state) => return forget(cloud, state),
    };
    let key = (config.provider, discriminator);
    let mut cached = lock(&cloud.account);
    if let Some(entry) = cached.as_ref().filter(|entry| entry.key == key) {
        return AccountState::Ready(entry.account.clone());
    }
    let state = match google {
        Some(credentials) => google_drive_account(data_root, credentials, cloud_http_transport()),
        None => local_folder_account(&config.local_folder),
    };
    *cached = match &state {
        AccountState::Ready(account) => Some(CachedAccount {
            key,
            account: account.clone(),
        }),
        _ => None,
    };
    state
}

/// What tells two accounts of one provider apart (folder path / account id), or the
/// non-ready state.
fn account_discriminator(data_root: &Path, config: &CloudConfig) -> Result<String, AccountState> {
    match config.provider {
        CloudProviderId::Off => Err(AccountState::Off),
        CloudProviderId::LocalFolder => match local_folder_account(&config.local_folder) {
            AccountState::Ready(_) => Ok(config.local_folder.clone()),
            other => Err(other),
        },
        CloudProviderId::GoogleDrive => google_account_id(data_root),
        http => Err(AccountState::Unavailable(http)),
    }
}

fn forget(cloud: &CloudSyncState, state: AccountState) -> AccountState {
    *lock(&cloud.account) = None;
    state
}

/// The Local folder account: folder chosen and present.
fn local_folder_account(folder: &str) -> AccountState {
    if folder.trim().is_empty() {
        return AccountState::NeedsSignIn("Choose the folder to sync with.");
    }
    let folder = PathBuf::from(folder);
    if !folder.is_dir() {
        return AccountState::NeedsSignIn("The cloud folder is not reachable.");
    }
    AccountState::Ready(local_folder(&folder))
}

/// A Local folder account over `folder` (also used by tests).
pub fn local_folder(folder: &Path) -> CloudAccount {
    let provider = LocalFolderProvider::new(folder);
    CloudAccount {
        label: provider.account_label(),
        account_key: account_key(
            CloudProviderId::LocalFolder.as_str(),
            &local_folder_account_id(folder),
        ),
        provider: Arc::new(provider),
    }
}

/// Readable provider name for messages.
pub fn provider_name(id: CloudProviderId) -> &'static str {
    match id {
        CloudProviderId::Off => "Off",
        CloudProviderId::GoogleDrive => "Google Drive",
        CloudProviderId::Onedrive => "OneDrive",
        CloudProviderId::Dropbox => "Dropbox",
        CloudProviderId::LocalFolder => "Local folder",
    }
}
