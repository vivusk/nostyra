//! Cloud saves (M19): what a sync unit is (one game's save folder keyed by system +
//! content sha1, or the shared PS2 memory cards), which files take part
//! ([`SyncScope`]), the remote manifest (the commit point: immutable per-generation
//! blobs, manifest written last), the per-account base manifest (last synced state),
//! the three-way planner, the executor that applies a plan safely
//! ([`sync_unit`]: staged + verified pulls, backups, hard-linked pushes, restarts on
//! lost races, `index.json`, blob GC) and the storage providers. Shared OAuth
//! plumbing for the HTTP providers: PKCE + loopback sign-in, token refresh, the
//! DPAPI refresh-token file, client ids and the [`HttpTransport`] seam. No Tauri here.

// The only exception: the two DPAPI calls in `token_store_dpapi` (`#[allow]` per fn).
#![deny(unsafe_code)]

mod base_manifest_store;
mod cloud_app_client_ids;
mod cloud_error;
mod cloud_index;
mod cloud_storage_provider;
mod cloud_sync_error;
mod conflict_choice;
mod content_hash;
mod device_identity;
#[cfg(any(test, feature = "test-support"))]
pub mod fake_http_transport;
pub mod http_transport;
mod local_file_snapshot;
mod local_save_backups;
mod manifest_path_safety;
mod metadata_read_cap;
mod oauth_endpoints;
mod oauth_error;
mod oauth_loopback_listener;
mod oauth_pkce;
mod oauth_sign_in;
mod oauth_token_client;
mod oauth_token_session;
pub mod providers;
mod remote_blob_gc;
mod remote_case_alias_filter;
mod secret_string;
mod sync_base_update;
mod sync_commit_finish;
mod sync_download;
mod sync_manifest;
mod sync_outcome;
mod sync_plan_executor;
mod sync_plan_executor_pull;
mod sync_plan_executor_push;
mod sync_progress;
mod sync_remote_state;
mod sync_scope;
mod sync_scope_maps;
mod sync_staging;
mod sync_unit;
mod sync_upload;
mod three_way_sync_planner;
#[cfg(windows)]
mod token_store_dpapi;
mod token_store_files;

#[cfg(test)]
mod sync_executor_cancel_tests;
#[cfg(test)]
mod sync_executor_fault_tests;
#[cfg(test)]
mod sync_executor_gc_tests;
#[cfg(test)]
mod sync_executor_pull_only_tests;
#[cfg(test)]
mod sync_executor_race_tests;
#[cfg(test)]
mod sync_executor_retry_tests;
#[cfg(test)]
mod sync_executor_tests;
#[cfg(test)]
mod sync_test_harness;

pub use base_manifest_store::{
    account_key, local_folder_account_id, BaseManifest, BaseManifestStore, LocalStat, RemoteBlobRef,
};
pub use cloud_app_client_ids::{
    client_credentials, client_credentials_with, ClientCredentials, OAuthProvider,
};
pub use cloud_error::CloudError;
pub use cloud_index::{
    changed_units, read_index, update_index, CloudIndex, IndexEntry, RepairRef, INDEX_PATH,
};
pub use cloud_storage_provider::{read_optional, CloudStorageProvider, ListEntry, Rev};
pub use cloud_sync_error::CloudSyncError;
pub use conflict_choice::{apply_choice, Choice};
pub use content_hash::{sha256_file, sha256_hex};
pub use device_identity::{current_computer_name, device_file, DeviceIdentity};
pub use http_transport::{HttpTransport, RequestBudget};
pub use local_file_snapshot::{snapshot_local_dir, LocalFile, LocalSnapshot};
pub use local_save_backups::{unit_backups_dir, BackupSide, BACKUPS_KEPT_PER_UNIT};
pub use oauth_endpoints::{
    encode_component, form_encode, LoopbackRedirect, OAuthEndpoints, PortRule, RedirectHost,
    TokenRevoke, DROPBOX_REDIRECT_PORTS, SIGN_IN_TIMEOUT,
};
pub use oauth_error::SignInError;
pub use oauth_loopback_listener::LoopbackListener;
pub use oauth_pkce::{random_state, s256_challenge, PkcePair};
pub use oauth_sign_in::{
    revoke, sign_in, sign_in_within, sign_out, AccountIdentity, SignedInAccount,
};
pub use oauth_token_client::{
    exchange_code, refresh_access_token, AccessToken, TokenGrant, REFRESH_MARGIN,
};
pub use oauth_token_session::{OAuthSession, RotatedTokenSink};
pub use secret_string::SecretString;
pub use sync_manifest::{
    blob_path, FileEntry, RemoteManifest, MANIFEST_FORMAT, TOMBSTONE_MAX_AGE_SECS,
};
pub use sync_outcome::{
    SyncContext, SyncDirection, SyncOptions, SyncOutcome, SyncStatus, SyncUnitError,
};
pub use sync_plan_executor::{sync_unit, MANIFEST_FILE};
pub use sync_progress::{SyncPhase, SyncProgress};
pub use sync_scope::{SyncScope, MAX_SYNC_FILE_BYTES};
pub use sync_scope_maps::{scope_maps, ScopedMaps};
pub use sync_staging::is_safe_relative_path;
pub use sync_unit::{SyncUnit, PS2_MEMCARDS_UNIT_KEY};
pub use sync_upload::LARGE_WRITE_BYTES;
pub use three_way_sync_planner::{effective_base, plan_sync, Conflict, ConflictKind, SyncPlan};
pub use token_store_files::{
    account_file, delete_account_files, load_account, load_refresh_token, refresh_token_file,
    save_signed_in_account, store_refresh_token, StoredAccount,
};
