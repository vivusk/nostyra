//! The base manifest: what this PC last synced for a unit (the remote manifest as
//! committed / read, plus the local size + mtime of each file at that moment), the
//! common ancestor of the three-way planner. One JSON file per unit and account:
//! `<data>/cloud/base/<account key>/<unit key>.json`. Switching account (or the
//! local folder) is a fresh base: first-sync rules, never deletes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cloud_sync_error::CloudSyncError;
use crate::content_hash::sha256_hex;
use crate::sync_manifest::RemoteManifest;
use crate::sync_unit::SyncUnit;

/// Local file stat recorded at the last sync: equal size + mtime → the file's sha is
/// the base entry's sha (no re-hash).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalStat {
    pub size: u64,
    /// Unix milliseconds.
    pub modified_ms: u64,
}

/// Last synced state of one unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseManifest {
    /// The remote manifest as last synced (generation, writer, entries + blob refs).
    /// Its live entries' shas are the base content of every file.
    pub manifest: RemoteManifest,
    /// Local stat per path at that sync. A missing stat (cleared) forces a re-hash.
    #[serde(default)]
    pub local: BTreeMap<String, LocalStat>,
    /// "Keep this PC" chosen before play (nothing uploaded then): the cloud copy each
    /// path will overwrite, backed up locally by the next full sync before its push.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub backup_remote_on_push: BTreeMap<String, RemoteBlobRef>,
}

/// A committed cloud file (blob under the unit folder + its content sha).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteBlobRef {
    pub blob: String,
    pub sha256: String,
}

impl BaseManifest {
    /// A base for `manifest` with no local stats (every file re-hashed next time).
    pub fn new(manifest: RemoteManifest) -> Self {
        Self {
            manifest,
            local: BTreeMap::new(),
            backup_remote_on_push: BTreeMap::new(),
        }
    }

    /// Base sha of `path` when it existed at the last sync (tombstones = absent).
    pub fn live_sha(&self, path: &str) -> Option<&str> {
        self.manifest.files.get(path).and_then(|e| e.live_sha())
    }
}

/// `<provider>-<16 hex of sha256(account id)>`: stable per account, no account
/// name in folder names.
pub fn account_key(provider_id: &str, account_id: &str) -> String {
    let hash = sha256_hex(account_id.as_bytes());
    format!("{provider_id}-{}", &hash[..16])
}

/// Account id of the Local folder provider: sha256 of the normalized folder path
/// (canonical when it exists; lower-case with `/` on Windows). Another folder is
/// another account, so it starts from a fresh base.
pub fn local_folder_account_id(folder: &Path) -> String {
    let path = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
    let mut text = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        text = text
            .trim_start_matches(r"\\?\")
            .replace('\\', "/")
            .to_lowercase();
    }
    let trimmed = text.trim_end_matches('/');
    sha256_hex(if trimmed.is_empty() { &text } else { trimmed }.as_bytes())
}

/// Base manifests of one account.
#[derive(Debug, Clone)]
pub struct BaseManifestStore {
    root: PathBuf,
}

impl BaseManifestStore {
    /// Store under `<data>/cloud/base/<account key>/`.
    pub fn new(data_root: &Path, account_key: &str) -> Self {
        Self {
            root: data_root.join("cloud").join("base").join(account_key),
        }
    }

    /// `<root>/<unit key>.json` (`<sys>/<sha1>.json` or `shared/<key>.json`).
    pub fn path_for(&self, unit: &SyncUnit) -> PathBuf {
        let key = unit.key();
        let mut path = self.root.clone();
        let mut parts = key.split('/').peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_some() {
                path.push(part);
            } else {
                path.push(format!("{part}.json"));
            }
        }
        path
    }

    /// The unit's base; `None` when there is none yet. An unreadable / corrupt base
    /// is also `None` (logged): first-sync rules can only conflict, never delete.
    pub fn load(&self, unit: &SyncUnit) -> Result<Option<BaseManifest>, CloudSyncError> {
        let path = self.path_for(unit);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(CloudSyncError::io(path, err)),
        };
        match serde_json::from_slice(&bytes) {
            Ok(base) => Ok(Some(base)),
            Err(err) => {
                tracing::warn!(target: "cloud_sync", "ignoring corrupt base {}: {err}", path.display());
                Ok(None)
            }
        }
    }

    /// Writes the unit's base atomically (temp file + rename).
    pub fn save(&self, unit: &SyncUnit, base: &BaseManifest) -> Result<(), CloudSyncError> {
        let path = self.path_for(unit);
        let bytes = serde_json::to_vec_pretty(base).map_err(|source| CloudSyncError::Json {
            what: "base manifest",
            source,
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| CloudSyncError::io(parent, err))?;
        }
        config_layers::write_atomically(&path, &bytes).map_err(|err| CloudSyncError::io(&path, err))
    }
}

#[cfg(test)]
#[path = "base_manifest_store_tests.rs"]
mod tests;
